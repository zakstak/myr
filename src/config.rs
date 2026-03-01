use std::env;

pub struct MyrConfig {
    pub saga_api_url: String,
    pub voice_api_key: String,
    pub saga_host: String,
    pub saga_voice_ip: String,
    pub saga_voice_port: String,
    pub myr_local_port: String,
}

impl MyrConfig {
    pub fn from_env() -> Self {
        let myr_local_port = get_env("MYR_LOCAL_PORT", "18765");
        let saga_api_url = env::var("SAGA_API_URL")
            .unwrap_or_else(|_| format!("http://localhost:{}", myr_local_port));

        Self {
            saga_api_url,
            voice_api_key: get_api_key(),
            saga_host: get_env("SAGA_HOST", "192.168.4.111"),
            saga_voice_ip: get_env("SAGA_VOICE_IP", "10.0.0.60"),
            saga_voice_port: get_env("SAGA_VOICE_PORT", "8765"),
            myr_local_port,
        }
    }
}

pub fn get_env(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn get_api_key() -> String {
    env::var("VOICE_API_KEY")
        .or_else(|_| env::var("SAGA_API_KEY"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_expected_defaults() {
        let cfg = MyrConfig::from_env();

        assert_eq!(cfg.saga_host, "192.168.4.111");
        assert_eq!(cfg.saga_voice_ip, "10.0.0.60");
        assert_eq!(cfg.saga_voice_port, "8765");
    }
}
