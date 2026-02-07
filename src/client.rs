#[cfg_attr(test, mockall::automock)]
pub trait SagaClient: Send + Sync {
    fn send_text(&self, text: &str) -> anyhow::Result<String>;
    fn send_audio(&self, wav_bytes: &[u8]) -> anyhow::Result<String>;
    fn health(&self) -> anyhow::Result<bool>;
}
