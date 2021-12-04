use chrono::{DateTime, Utc};
use handlebars::{handlebars_helper, Handlebars};
use lettre::{ClientSecurity, SendableEmail, SmtpClient, SmtpTransport, Transport};
use lettre_email::EmailBuilder;
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::notifs::Notify;
use crate::{ExecutionResult, JobResult};

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use log::debug;
    use pretty_assertions::assert_eq;
    use pretty_env_logger::try_init;
    use serde_json::json;

    use crate::notifs::mail::{render_text, Mailer};
    use crate::notifs::Notify;
    use crate::utils::git::CommitPerson;
    use crate::utils::tests::get_sample_resource_file;
    use crate::{Commit, ExecutionContext, ExecutionResult, JobResult};

    #[test]
    #[ignore]
    fn send_basic_success_mail() {
        let exec_res = ExecutionResult {
            job_results: vec![JobResult {
                name: "job 1".to_string(),
                success: true,
                logs: vec!["everything went well!".to_string()],
                start_date: Utc::now() - Duration::seconds(100),
                end_date: Utc::now(),
                ..Default::default()
            }],
            context: ExecutionContext {
                branch: "main".to_string(),
                commit: Commit {
                    author: CommitPerson {
                        name: "coincoin".to_string(),
                        email: "example@example.fr".to_string(),
                        date: Utc::now(),
                    },
                    ..Default::default()
                },
            },
            start_date: Utc::now() - Duration::seconds(100),
            end_date: Utc::now(),
        };

        let s = get_sample_resource_file("notifs/simple_smtp.yml")
            .expect("could not read simple_smtp.yml");

        let mailer: Mailer = serde_yaml::from_str(&s).expect("could not build mailer");
        assert_eq!(mailer.from, "fakeci@example.org");
        assert!(mailer.send(&exec_res).is_ok());
    }

    #[test]
    fn render_template() {
        let _ = try_init();
        let exec_res = ExecutionResult {
            job_results: vec![
                JobResult {
                    success: true,
                    name: "job1".to_string(),
                    start_date: Utc::now() - Duration::seconds(300),
                    end_date: Utc::now() - Duration::seconds(200),
                    logs: vec!["line 1".to_string(), "line 2".to_string()],
                },
                JobResult {
                    success: true,
                    name: "job2".to_string(),
                    start_date: Utc::now() - Duration::seconds(190),
                    end_date: Utc::now(),
                    logs: vec!["line 3".to_string(), "line 4".to_string()],
                },
            ],
            context: ExecutionContext {
                branch: "main".to_string(),
                commit: Default::default(),
            },
            start_date: Utc::now() - Duration::seconds(300),
            end_date: Utc::now(),
        };
        debug!("context: {:#?}", json!(exec_res));
        let s = render_text(&exec_res);
        debug!("result: {:#?}", s);
        assert!(s.is_ok());
        let s = s.unwrap();
        debug!("rendered template: \n{}", s);
    }
}

// TODO: handle auth (ssl brrr)
#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum SMTPAuth {
    None,
}

impl Default for SMTPAuth {
    fn default() -> Self {
        Self::None
    }
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SMTPConfig {
    pub(crate) addr: String,
    pub(crate) port: u16,
    #[serde(default = "SMTPAuth::default", skip_serializing_if = "is_default")]
    pub(crate) auth: SMTPAuth,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Mailer {
    pub(crate) from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recipients: Option<Vec<String>>,
    pub(crate) server: SMTPConfig,
}

fn render_text(ctx: &ExecutionResult) -> anyhow::Result<String> {
    let mut reg = Handlebars::new();
    handlebars_helper!(status: |job_results: Vec<JobResult>| {
        match job_results.iter().any(|r| !r.success) {
            true => "Failure",
            false => "Success",
        }
    });
    handlebars_helper!(duration: |start: DateTime<Utc>, end: DateTime<Utc>| {
        format!("{}", (end - start).num_seconds())
    });
    reg.register_helper("build_status", Box::new(status));
    reg.register_helper("duration", Box::new(duration));
    Ok(reg.render_template(
        include_str!("../../../resources/templates/notifs/mail.txt.hbs"),
        &json!(ctx),
    )?)
}

impl Notify for Mailer {
    fn send(&self, exec_res: &ExecutionResult) -> anyhow::Result<()> {
        let mut email = EmailBuilder::new().to(exec_res.context.commit.author.email.as_str());
        if let Some(recipients) = &self.recipients {
            for recipient in recipients {
                debug!("Adding {} to recipients", recipient);
                email = email.to(recipient.to_string());
            }
        }
        let email = email
            .from(self.from.as_str())
            .subject(format!(
                "build results for {}: {}",
                exec_res.context.branch,
                match exec_res.job_results.iter().any(|r| !r.success) {
                    false => "Success!",
                    true => "Failure",
                }
            ))
            .text(render_text(exec_res)?)
            .build()
            .expect("Error while building mail!");
        let mut mailer = SmtpTransport::new(SmtpClient::new(
            format!("{}:{}", self.server.addr, self.server.port),
            ClientSecurity::None,
        )?);
        let _ = mailer.send(SendableEmail::try_from(email)?)?;
        Ok(())
    }
}
