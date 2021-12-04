use anyhow::Result;

use crate::ExecutionResult;

#[cfg(feature = "mails")]
pub mod mail;

pub trait Notify {
    fn send(self, exec_res: &ExecutionResult) -> Result<()>;
}
