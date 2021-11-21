use std::env;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use anyhow::Result;
use log::{debug, error, info};
use tempdir::TempDir;
use crate::conf::FakeCIRepoConfig;
use serde::Serialize;

mod utils;
mod conf;

#[cfg(test)]
mod tests {
    use std::fs::{remove_file};
    use std::path::Path;
    use crate::{execute_config, launch};

    #[test]
    fn test_hello_world() {
        pretty_env_logger::try_init();
        let conf = "pipeline:
  - name: \"hello world\"
    steps:
      - name: \"Create File\"
        execute:
          - \"touch /tmp/hello_world\"";
        let config = serde_yaml::from_str(conf).unwrap();
        assert!(execute_config(config).is_ok());
        let p = Path::new("/tmp/hello_world");
        assert!(p.is_file());
        remove_file(p).expect("Could not remove file in test_hello_world");
    }

    #[test]
    fn test_dog_feeding() {
        pretty_env_logger::try_init();
        let r = launch(".");
    }
}

type JobsLogs = Vec<String>;
#[derive(Default, Serialize)]
pub struct ExecutionResult {
    pub logs: Vec<JobsLogs>,
    pub artifacts: Vec<String>
}


fn execute_config(conf: FakeCIRepoConfig) -> Result<ExecutionResult> {
    let mut e = ExecutionResult::default();
    for job in conf.pipeline {
        for step in job.steps {
            let mut logs: Vec<String> = Vec::new();
            for e in step.execute {
                info!("Running step {}", step.name);
                let output = Command::new("bash").args(["-c", &e]).envs(&job.env.clone().unwrap_or_default()).output()?;
                if output.stdout.len() > 0 {
                    let s = String::from_utf8_lossy(&output.stdout);
                    debug!("stdout: {}", &s);
                    logs.push(s.to_string());
                }
                if output.stderr.len() > 0 {
                    let s = String::from_utf8_lossy(&output.stderr);
                    debug!("stderr: {}", &s);
                    logs.push(s.to_string());
                }
                if !output.status.success(){
                    error!("Step {} returned execution failure! aborting next steps", step.name);
                    logs.push(format!("Step {} returned execution failure! aborting next steps", step.name));
                    break;
                }
            }
            e.logs.push(logs);
        }
    }
    Ok(e)
}

fn execute_from_file(path: &Path) -> Result<ExecutionResult> {
    let c = match serde_yaml::from_reader(File::open(path)?){
        Ok(c) => c,
        Err(e) => {
            error!("Could not parse yaml config: {}", e);
            panic!();
        }
    };
    let r = execute_config(c)?;
    Ok(r)
}

pub fn launch(repo_url: &str) -> Result<ExecutionResult> {
    let root = TempDir::new("fakeci_execution")?;
    let output = Command::new("git").args(["clone", repo_url, root.path().to_str().expect("Could not convert from tmpdir to str")]).output()?;
    if !output.status.success() {
        error!("Could not clone repo!!!!!!!S");
        panic!();
    }
    let old_path = env::current_dir()?;
    env::set_current_dir(root.path())?;
    let r = execute_from_file(Path::new(".fakeci.yml"))?;
    env::set_current_dir(old_path)?;
    Ok(r)
}