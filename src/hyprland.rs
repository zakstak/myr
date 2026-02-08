use crate::dsl::Command;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[cfg_attr(test, mockall::automock)]
pub trait HyprlandExecutor: Send + Sync {
    fn execute(&self, cmd: &Command) -> anyhow::Result<()>;
    fn get_active_window(&self) -> anyhow::Result<String>;
    fn list_windows(&self) -> anyhow::Result<Vec<String>>;
}

pub struct RealHyprlandExecutor {
    socket_path: PathBuf,
}

impl RealHyprlandExecutor {
    pub fn new() -> anyhow::Result<Self> {
        let his = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| {
            anyhow::anyhow!("HYPRLAND_INSTANCE_SIGNATURE not set — is Hyprland running?")
        })?;

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());

        let socket_path = PathBuf::from(&runtime_dir)
            .join("hypr")
            .join(&his)
            .join(".socket.sock");

        if !socket_path.exists() {
            anyhow::bail!(
                "Hyprland socket not found at {:?} — is Hyprland running?",
                socket_path
            );
        }

        Ok(Self { socket_path })
    }

    fn send_request(&self, request: &str) -> anyhow::Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| anyhow::anyhow!("Failed to connect to Hyprland socket: {}", e))?;

        stream.write_all(request.as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    }
}

impl HyprlandExecutor for RealHyprlandExecutor {
    fn execute(&self, cmd: &Command) -> anyhow::Result<()> {
        let hyprctl_cmd = cmd.to_hyprctl();
        let response = self.send_request(&hyprctl_cmd)?;

        if response.trim() == "ok" || response.trim().is_empty() {
            Ok(())
        } else {
            anyhow::bail!("Hyprland dispatch failed: {}", response.trim())
        }
    }

    fn get_active_window(&self) -> anyhow::Result<String> {
        let response = self.send_request("j/activewindow")?;
        let v: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse active window JSON: {}", e))?;

        v.get("title")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No title in active window response"))
    }

    fn list_windows(&self) -> anyhow::Result<Vec<String>> {
        let response = self.send_request("j/clients")?;
        let clients: Vec<serde_json::Value> = serde_json::from_str(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse clients JSON: {}", e))?;

        let mut windows = Vec::new();
        for client in &clients {
            let class = client.get("class").and_then(|c| c.as_str()).unwrap_or("");
            let title = client.get("title").and_then(|t| t.as_str()).unwrap_or("");
            windows.push(format!("{} — {}", class, title));
        }

        Ok(windows)
    }
}
