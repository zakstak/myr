use std::process::Command;

pub fn get_clients() -> anyhow::Result<String> {
    let output = Command::new("hyprctl")
        .arg("clients")
        .arg("-j")
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("hyprctl failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(String::from_utf8(output.stdout)?)
}

pub fn dispatch(command: &str) -> anyhow::Result<()> {
    // command is like "movetoworkspace 2,address:0x1234" or "workspace 2"
    // hyprctl dispatch expects: hyprctl dispatch <dispatcher> <args>

    let parts: Vec<&str> = command.trim().splitn(2, ' ').collect();
    if parts.is_empty() {
        return Ok(());
    }

    let dispatcher = parts[0];
    let args = if parts.len() > 1 { parts[1] } else { "" };

    let status = Command::new("hyprctl")
        .arg("dispatch")
        .arg(dispatcher)
        .arg(args)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("hyprctl dispatch failed for: {}", command));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_parsing() {
        // We can't really test the Command execution, but we can test logic if we separated it.
        // For now, this is just a placeholder.
        assert!(true);
    }
}
