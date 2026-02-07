use crate::dsl::Command;

#[cfg_attr(test, mockall::automock)]
pub trait HyprlandExecutor: Send + Sync {
    fn execute(&self, cmd: &Command) -> anyhow::Result<()>;
    fn get_active_window(&self) -> anyhow::Result<String>;
    fn list_windows(&self) -> anyhow::Result<Vec<String>>;
}
