use anyhow::Result;

use crate::ExecutionResult;

pub mod mail;

pub trait Notify {
    fn send(exec_res: &ExecutionResult, commit_author: &str) -> Result<()>;
}
