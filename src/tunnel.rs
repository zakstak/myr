use anyhow::{Context, Result};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

pub struct TunnelConfig {
    pub saga_host: String,
    pub saga_voice_ip: String,
    pub saga_voice_port: String,
    pub local_port: String,
}

pub struct SshTunnel {
    process: Option<Child>,
}

impl SshTunnel {
    pub fn establish(config: TunnelConfig) -> Result<Self> {
        let health_url = format!("http://localhost:{}/health", config.local_port);

        // Check if already reachable
        if Self::check_health(&health_url) {
            tracing::info!("Tunnel already reachable at {}", health_url);
            return Ok(Self { process: None });
        }

        tracing::info!("Setting up SSH tunnel via {}...", config.saga_host);

        // Kill any existing tunnel on this port
        let pkill_pattern = format!(
            "ssh.*-L.*{}:{}:{}",
            config.local_port, config.saga_voice_ip, config.saga_voice_port
        );
        let _ = Command::new("pkill").args(["-f", &pkill_pattern]).output();

        // Create tunnel in background
        let bind_spec = format!(
            "{}:{}:{}",
            config.local_port, config.saga_voice_ip, config.saga_voice_port
        );
        let ssh_target = format!("root@{}", config.saga_host);

        let child = Command::new("ssh")
            .args(["-f", "-N", "-L", &bind_spec, &ssh_target])
            .spawn()
            .context("Failed to spawn SSH tunnel")?;

        // Wait for tunnel to establish
        thread::sleep(Duration::from_secs(1));

        // Verify tunnel is working
        if !Self::check_health(&health_url) {
            return Err(anyhow::anyhow!(
                "Cannot reach Voice API at {}. Check SSH connection.",
                health_url
            ));
        }

        tracing::info!("Tunnel established");
        Ok(Self {
            process: Some(child),
        })
    }

    pub fn teardown(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            child.kill().context("Failed to kill SSH process")?;
            child.wait().context("Failed to wait for SSH process")?;
            tracing::info!("Tunnel torn down");
        }
        Ok(())
    }

    fn check_health(url: &str) -> bool {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok();

        if let Some(c) = client {
            c.get(url)
                .send()
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        } else {
            false
        }
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_config_construction() {
        let config = TunnelConfig {
            saga_host: "192.168.4.111".to_string(),
            saga_voice_ip: "10.0.0.60".to_string(),
            saga_voice_port: "8765".to_string(),
            local_port: "18765".to_string(),
        };

        assert_eq!(config.saga_host, "192.168.4.111");
        assert_eq!(config.saga_voice_ip, "10.0.0.60");
        assert_eq!(config.saga_voice_port, "8765");
        assert_eq!(config.local_port, "18765");
    }

    #[test]
    fn test_tunnel_drop_cleanup() {
        // Create a mock tunnel without actually establishing SSH
        let mut tunnel = SshTunnel { process: None };

        // Test that teardown works on None process
        assert!(tunnel.teardown().is_ok());

        // Drop should not panic
        drop(tunnel);
    }

    #[test]
    fn test_check_health_unreachable() {
        // Test health check against unreachable endpoint
        let reachable = SshTunnel::check_health("http://localhost:99999/health");
        assert!(!reachable);
    }
}
