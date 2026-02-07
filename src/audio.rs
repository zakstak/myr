#[cfg_attr(test, mockall::automock)]
pub trait AudioCapture: Send + Sync {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<Vec<u8>>;
    fn is_recording(&self) -> bool;
}
