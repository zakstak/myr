use std::env;

pub struct MyrConfig {
    pub saga_api_url: String,
    pub saga_api_key: String,
    pub hyprland_socket: String,
    pub audio_device: String,
}

impl MyrConfig {
    pub fn from_env() -> Self {
        Self {
            saga_api_url: get_env("SAGA_API_URL", "http://localhost:8765"),
            saga_api_key: get_env("SAGA_API_KEY", ""),
            hyprland_socket: get_env("HYPRLAND_INSTANCE_SIGNATURE", ""),
            audio_device: get_env("MYR_AUDIO_DEVICE", "default"),
        }
    }
}

pub fn get_env(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
