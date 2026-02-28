use crate::config::MyrConfig;
use anyhow::{Context, Result};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct DictateResponse {
    pub raw: String,
    pub refined: String,
    pub latency_ms: u64,
}

#[cfg_attr(test, mockall::automock)]
pub trait SagaClient: Send + Sync {
    fn send_text(&self, text: &str, context: &str) -> anyhow::Result<String>;
    fn send_audio(&self, wav_bytes: &[u8], context: &str) -> anyhow::Result<String>;
    fn send_dictate(
        &self,
        audio_data: &[u8],
        dictionary: &HashMap<String, String>,
        personal_dictionary: &HashMap<String, String>,
        context: &str,
    ) -> anyhow::Result<DictateResponse>;
    fn health(&self) -> anyhow::Result<bool>;
}

pub struct RealSagaClient {
    client: Client,
    api_url: String,
    api_key: String,
}

impl RealSagaClient {
    pub fn new(config: &MyrConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            api_url: config.saga_api_url.clone(),
            api_key: config.saga_api_key.clone(),
        })
    }

    fn send_request_once(&self, form: Form, endpoint: &str) -> Result<String> {
        let url = format!("{}/{}", self.api_url, endpoint);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .multipart(form)
            .send()
            .context("HTTP request failed")?;

        let status = response.status();
        let body = response.text().context("Failed to read response body")?;

        tracing::debug!("API response status={} body={}", status, body);

        let json: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse JSON response")?;

        if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
            anyhow::bail!("API error: {}", err);
        }

        json.get("commands")
            .or_else(|| json.get("text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing commands/text in response: {}", body))
    }
}

impl SagaClient for RealSagaClient {
    fn send_text(&self, text: &str, context: &str) -> Result<String> {
        for attempt in 1..=2 {
            let form = Form::new()
                .text("text", text.to_string())
                .text("context", context.to_string());

            let result = self.send_request_once(form, "command");

            match result {
                Ok(resp) => return Ok(resp),
                Err(e) if attempt == 1 => {
                    tracing::warn!("First attempt failed: {}, retrying once...", e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        unreachable!()
    }

    fn send_audio(&self, wav_bytes: &[u8], context: &str) -> Result<String> {
        for attempt in 1..=2 {
            let audio_part = Part::bytes(wav_bytes.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .context("Failed to set MIME type")?;

            let form = Form::new()
                .part("audio", audio_part)
                .text("context", context.to_string());

            let result = self.send_request_once(form, "command");

            match result {
                Ok(resp) => return Ok(resp),
                Err(e) if attempt == 1 => {
                    tracing::warn!("First attempt failed: {}, retrying once...", e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        unreachable!()
    }

    fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.api_url);

        let response = self.client.get(&url).timeout(Duration::from_secs(2)).send();

        Ok(response.map(|r| r.status().is_success()).unwrap_or(false))
    }

    fn send_dictate(
        &self,
        audio_data: &[u8],
        dictionary: &HashMap<String, String>,
        personal_dictionary: &HashMap<String, String>,
        context: &str,
    ) -> Result<DictateResponse> {
        let audio_part = Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("Failed to set MIME type")?;

        let dict_json =
            serialize_developer_dictionary(dictionary).context("Failed to serialize dictionary")?;
        let personal_dict_json = serde_json::to_string(personal_dictionary)
            .context("Failed to serialize personal dictionary")?;

        let form = Form::new()
            .part("audio", audio_part)
            .text("dictionary", dict_json)
            .text("personal_dictionary", personal_dict_json)
            .text("context", context.to_string());

        let url = format!("{}/dictate", self.api_url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .multipart(form)
            .send()
            .context("Dictate HTTP request failed")?;

        let status = response.status();
        let body = response
            .text()
            .context("Failed to read dictate response body")?;

        tracing::debug!("Dictate response status={} body={}", status, body);

        if !status.is_success() {
            anyhow::bail!("Dictate API returned status {}: {}", status, body);
        }

        serde_json::from_str::<DictateResponse>(&body)
            .context("Failed to parse DictateResponse JSON")
    }
}

fn serialize_developer_dictionary(dictionary: &HashMap<String, String>) -> Result<String> {
    let mut terms: Vec<String> = dictionary.values().cloned().collect();
    terms.sort();
    terms.dedup();
    serde_json::to_string(&terms).context("Failed to serialize developer dictionary terms")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_mock_saga_client_send_text() {
        let mut mock = MockSagaClient::new();
        mock.expect_send_text()
            .with(
                mockall::predicate::eq("focus terminal"),
                mockall::predicate::always(),
            )
            .times(1)
            .returning(|_, _| Ok("FOCUS class:kitty".to_string()));

        let result = mock.send_text("focus terminal", "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "FOCUS class:kitty");
    }

    #[test]
    fn test_mock_saga_client_send_audio() {
        let mut mock = MockSagaClient::new();
        let wav_data = vec![0u8; 1024];

        mock.expect_send_audio()
            .times(1)
            .returning(|_, _| Ok("CLOSE title:Slack".to_string()));

        let result = mock.send_audio(&wav_data, "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "CLOSE title:Slack");
    }

    #[test]
    fn test_mock_saga_client_health() {
        let mut mock = MockSagaClient::new();

        mock.expect_health().times(1).returning(|| Ok(true));

        let result = mock.health();
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_mock_saga_client_send_dictate() {
        let mut mock = MockSagaClient::new();
        let wav_data = vec![0u8; 512];

        mock.expect_send_dictate().times(1).returning(|_, _, _, _| {
            Ok(DictateResponse {
                raw: "hello world".to_string(),
                refined: "Hello, world!".to_string(),
                latency_ms: 150,
            })
        });

        let dict = HashMap::from([("kubernetes".to_string(), "Kubernetes".to_string())]);
        let personal = HashMap::new();

        let result = mock.send_dictate(&wav_data, &dict, &personal, "coding");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.raw, "hello world");
        assert_eq!(resp.refined, "Hello, world!");
        assert_eq!(resp.latency_ms, 150);
    }

    #[test]
    fn test_serialize_developer_dictionary_as_json_string_array() {
        let dictionary = HashMap::from([
            ("nicks".to_string(), "NixOS".to_string()),
            ("kube".to_string(), "Kubernetes".to_string()),
            ("k8s".to_string(), "Kubernetes".to_string()),
        ]);

        let serialized = serialize_developer_dictionary(&dictionary).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        let terms = parsed.as_array().unwrap();
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].as_str().unwrap(), "Kubernetes");
        assert_eq!(terms[1].as_str().unwrap(), "NixOS");
    }

    #[test]
    fn test_personal_dictionary_serializes_as_json_object_mapping() {
        let personal = HashMap::from([
            ("nicks".to_string(), "NixOS".to_string()),
            ("dock compose".to_string(), "Docker Compose".to_string()),
        ]);

        let serialized = serde_json::to_string(&personal).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        let obj = parsed.as_object().unwrap();
        assert_eq!(obj.get("nicks").and_then(|v| v.as_str()), Some("NixOS"));
        assert_eq!(
            obj.get("dock compose").and_then(|v| v.as_str()),
            Some("Docker Compose")
        );
    }

    #[test]
    fn test_mock_saga_client_error() {
        let mut mock = MockSagaClient::new();

        mock.expect_send_text()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("Connection timeout")));

        let result = mock.send_text("test", "");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Connection timeout"));
    }
}
