use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub struct MyrConfig {
    pub saga_api_url: String,
    pub saga_api_key: String,
    pub hyprland_socket: String,
    pub audio_device: String,
    pub saga_host: String,
    pub saga_voice_ip: String,
    pub saga_voice_port: String,
    pub myr_local_port: String,
}

impl MyrConfig {
    pub fn from_env() -> Self {
        Self {
            saga_api_url: get_env("SAGA_API_URL", "http://localhost:18765"),
            saga_api_key: get_env("SAGA_API_KEY", ""),
            hyprland_socket: get_env("HYPRLAND_INSTANCE_SIGNATURE", ""),
            audio_device: get_env("MYR_AUDIO_DEVICE", "default"),
            saga_host: get_env("SAGA_HOST", "192.168.4.111"),
            saga_voice_ip: get_env("SAGA_VOICE_IP", "10.0.0.60"),
            saga_voice_port: get_env("SAGA_VOICE_PORT", "8765"),
            myr_local_port: get_env("MYR_LOCAL_PORT", "18765"),
        }
    }
}

pub fn get_env(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

#[derive(Debug, Deserialize)]
struct DictionaryFile {
    #[serde(default)]
    terms: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Snippet {
    pub trigger: String,
    pub expand: String,
}

#[derive(Debug, Deserialize)]
struct SnippetsFile {
    #[serde(default)]
    snippet: Vec<Snippet>,
}

/// Re-read on every dictation trigger (never cached in daemon state).
#[derive(Debug, Clone)]
pub struct DictationConfig {
    pub developer_dictionary: HashMap<String, String>,
    pub personal_dictionary: HashMap<String, String>,
    pub snippets: Vec<Snippet>,
}

impl DictationConfig {
    fn config_dir() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".config/saga/voice")
    }

    pub fn load() -> Self {
        let dir = Self::config_dir();

        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
            Self::write_defaults(&dir);
        }

        let developer_dictionary = Self::load_dictionary(dir.join("developer-dictionary.toml"));
        let personal_dictionary = Self::load_dictionary(dir.join("personal-dictionary.toml"));
        let snippets = Self::load_snippets(dir.join("snippets.toml"));

        Self {
            developer_dictionary,
            personal_dictionary,
            snippets,
        }
    }

    fn load_dictionary(path: PathBuf) -> HashMap<String, String> {
        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<DictionaryFile>(&content) {
                Ok(file) => file.terms,
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    HashMap::new()
                }
            },
            Err(_) => HashMap::new(),
        }
    }

    fn load_snippets(path: PathBuf) -> Vec<Snippet> {
        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<SnippetsFile>(&content) {
                Ok(file) => file.snippet,
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        }
    }

    fn write_defaults(dir: &PathBuf) {
        let dict_default = "# Dictionary — spoken form → written form\n\n[terms]\n";
        let snippets_default =
            "# Voice-triggered snippets\n# [[snippet]]\n# trigger = \":sig\"\n# expand = \"Best regards\"\n";

        let _ = fs::write(dir.join("developer-dictionary.toml"), dict_default);
        let _ = fs::write(dir.join("personal-dictionary.toml"), dict_default);
        let _ = fs::write(dir.join("snippets.toml"), snippets_default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_dictation_config_load_empty_dir() {
        let tmp = std::env::temp_dir().join("myr_test_dictation_config");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(tmp.join("developer-dictionary.toml"), "[terms]\n").unwrap();
        fs::write(tmp.join("personal-dictionary.toml"), "[terms]\n").unwrap();
        fs::write(tmp.join("snippets.toml"), "").unwrap();

        let dev = DictationConfig::load_dictionary(tmp.join("developer-dictionary.toml"));
        let personal = DictationConfig::load_dictionary(tmp.join("personal-dictionary.toml"));
        let snippets = DictationConfig::load_snippets(tmp.join("snippets.toml"));

        assert!(dev.is_empty());
        assert!(personal.is_empty());
        assert!(snippets.is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_dictation_config_load_with_data() {
        let tmp = std::env::temp_dir().join("myr_test_dictation_config_data");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("developer-dictionary.toml"),
            "[terms]\nkubernetes = \"Kubernetes\"\nnixos = \"NixOS\"\n",
        )
        .unwrap();
        fs::write(tmp.join("personal-dictionary.toml"), "[terms]\n").unwrap();
        fs::write(
            tmp.join("snippets.toml"),
            "[[snippet]]\ntrigger = \":sig\"\nexpand = \"Best regards\"\n",
        )
        .unwrap();

        let dev = DictationConfig::load_dictionary(tmp.join("developer-dictionary.toml"));
        let snippets = DictationConfig::load_snippets(tmp.join("snippets.toml"));

        assert_eq!(dev.len(), 2);
        assert_eq!(dev.get("kubernetes").unwrap(), "Kubernetes");
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].trigger, ":sig");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_dictation_config_missing_file_returns_empty() {
        let result = DictationConfig::load_dictionary(PathBuf::from("/nonexistent/path/dict.toml"));
        assert!(result.is_empty());

        let snippets =
            DictationConfig::load_snippets(PathBuf::from("/nonexistent/path/snippets.toml"));
        assert!(snippets.is_empty());
    }
}
