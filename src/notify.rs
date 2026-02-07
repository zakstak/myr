#[cfg_attr(test, mockall::automock)]
pub trait Notifier: Send + Sync {
    fn notify(&self, summary: &str, body: &str) -> anyhow::Result<()>;
    fn error(&self, summary: &str, body: &str) -> anyhow::Result<()>;
}
