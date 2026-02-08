use crate::config::MyrConfig;
use anyhow::{Context, Result};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use std::time::Duration;

#[cfg_attr(test, mockall::automock)]
pub trait SagaClient: Send + Sync {
    fn send_text(&self, text: &str) -> anyhow::Result<String>;
    fn send_audio(&self, wav_bytes: &[u8]) -> anyhow::Result<String>;
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

        let json: serde_json::Value = response.json().context("Failed to parse JSON response")?;

        json.get("commands")
            .or_else(|| json.get("text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing commands/text in response"))
    }
}

impl SagaClient for RealSagaClient {
    fn send_text(&self, text: &str) -> Result<String> {
        let context = "";

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

    fn send_audio(&self, wav_bytes: &[u8]) -> Result<String> {
        let context = "";

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_saga_client_send_text() {
        let mut mock = MockSagaClient::new();
        mock.expect_send_text()
            .with(mockall::predicate::eq("focus terminal"))
            .times(1)
            .returning(|_| Ok("FOCUS class:kitty".to_string()));

        let result = mock.send_text("focus terminal");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "FOCUS class:kitty");
    }

    #[test]
    fn test_mock_saga_client_send_audio() {
        let mut mock = MockSagaClient::new();
        let wav_data = vec![0u8; 1024];

        mock.expect_send_audio()
            .times(1)
            .returning(|_| Ok("CLOSE title:Slack".to_string()));

        let result = mock.send_audio(&wav_data);
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
    fn test_mock_saga_client_error() {
        let mut mock = MockSagaClient::new();

        mock.expect_send_text()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("Connection timeout")));

        let result = mock.send_text("test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Connection timeout"));
    }
}
