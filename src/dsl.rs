use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verb {
    Focus,
    Close,
    Move,
    Resize,
    Open,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selector {
    Active,
    Title(String),
    Class(String),
    Index(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub verb: Verb,
    pub selector: Option<Selector>,
    pub args: Vec<String>,
}

pub fn parse(_input: &str) -> anyhow::Result<Command> {
    anyhow::bail!("DSL parser not yet implemented")
}
