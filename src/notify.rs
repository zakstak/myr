use std::process::Command;
use tracing::{error, info};

#[cfg_attr(test, mockall::automock)]
pub trait Notifier: Send + Sync {
    fn notify(&self, summary: &str, body: &str) -> anyhow::Result<()>;
    fn error(&self, summary: &str, body: &str) -> anyhow::Result<()>;
}

pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, summary: &str, body: &str) -> anyhow::Result<()> {
        info!(summary, body, "Sending desktop notification");
        Command::new("notify-send")
            .arg("--app-name=myr")
            .arg("--urgency=low")
            .arg(summary)
            .arg(body)
            .spawn()
            .map_err(|e| {
                error!(error = %e, "Failed to spawn notify-send");
                anyhow::anyhow!("Failed to spawn notify-send: {}", e)
            })?;
        Ok(())
    }

    fn error(&self, summary: &str, body: &str) -> anyhow::Result<()> {
        error!(summary, body, "Sending desktop error notification");
        Command::new("notify-send")
            .arg("--app-name=myr")
            .arg("--urgency=critical")
            .arg(summary)
            .arg(body)
            .spawn()
            .map_err(|e| {
                error!(error = %e, "Failed to spawn notify-send");
                anyhow::anyhow!("Failed to spawn notify-send: {}", e)
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_notifier_trait_compliance() {
        let notifier = DesktopNotifier;
        let _: &dyn Notifier = &notifier;
    }

    #[test]
    fn test_mock_notifier() {
        let mut mock = MockNotifier::new();
        mock.expect_notify()
            .with(
                mockall::predicate::eq("test"),
                mockall::predicate::eq("body"),
            )
            .times(1)
            .returning(|_, _| Ok(()));

        mock.notify("test", "body").unwrap();
    }
}
