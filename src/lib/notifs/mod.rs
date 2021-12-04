use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "mails")]
use crate::notifs::mail::Mailer;
use crate::ExecutionResult;

#[cfg(feature = "mails")]
pub mod mail;

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum Notifier {
    #[cfg(feature = "mails")]
    Mailer(Mailer),
}

impl Notifier {
    pub fn send(&self, exec_res: &ExecutionResult) -> Result<()> {
        match self {
            Notifier::Mailer(e) => e.send(exec_res),
        }
    }
}

pub trait Notify {
    fn send(&self, exec_res: &ExecutionResult) -> Result<()>;
}
