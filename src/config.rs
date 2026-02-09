use std::env;

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
