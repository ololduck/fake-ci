use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "mails")]
use crate::notifications::mail::Mailer;
use crate::ExecutionResult;

#[cfg(feature = "mails")]
/// Mail notifications
pub mod mail;

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
/// Represents all possible notifiers
pub enum Notifier {
    #[cfg(feature = "mails")]
    /// Sending mails with SMTP
    Mailer(Mailer),
}

impl Notifier {
    /// Sends the communication, whatever the variant of Notifier it is
    pub fn send(&self, exec_res: &ExecutionResult) -> Result<()> {
        match self {
            Notifier::Mailer(e) => e.send(exec_res),
        }
    }
}

/// Defines a [Notifier], who can communicate build results to the outside world
pub trait Notify {
    /// validates the intention to communicate the result to the outside world
    fn send(&self, exec_res: &ExecutionResult) -> Result<()>;
}
