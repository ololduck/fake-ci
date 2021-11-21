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
        let _ = pretty_env_logger::try_init();
        let conf = "pipeline:
  - name: \"hello world\"
    steps:
      - name: \"Create File\"
        exec:
          - \"touch /tmp/hello_world\"";
        let config = serde_yaml::from_str(conf).unwrap();
        assert!(execute_config(config).is_ok());
        let p = Path::new("/tmp/hello_world");
        assert!(p.is_file());
        remove_file(p).expect("Could not remove file in test_hello_world");
    }

    #[test]
    fn test_dog_feeding() {
        let _ = pretty_env_logger::try_init();
        let r = launch(".");
        assert!(r.is_ok());
        let r = r.unwrap();
        for job_result in r.job_results {
            assert_eq!(job_result.success, true);
        }
    }
}

#[derive(Default, Serialize)]
pub struct JobResult {
    pub success: bool,
    pub logs: Vec<String>
}
#[derive(Default, Serialize)]
pub struct ExecutionResult {
    pub job_results: Vec<JobResult>,
    pub artifacts: Vec<String>
}


fn execute_config(conf: FakeCIRepoConfig) -> Result<ExecutionResult> {
    let mut e = ExecutionResult::default();
    for job in conf.pipeline {
        info!("Running job \"{}\"", job.name);
        let mut logs: Vec<String> = Vec::new();
        let mut result = JobResult{
            success: true,
            ..Default::default()
        };
        for step in job.steps {
            let mut step_counter = 0;
            let s_name = step.name.unwrap_or(step_counter.to_string());
            info!(" Running step \"{}\"", s_name);
            for e in step.exec {
                info!("  - {}", e);
                let output = Command::new("bash").args(["-c", &e]).envs(&job.env.clone().unwrap_or_default()).output()?;
                if output.stdout.len() > 0 {
                    let s = String::from_utf8_lossy(&output.stdout);
                    let _ = &s.lines().map(|l| debug!("    stdout: {}", l)).collect::<Vec<_>>();
                    result.logs.push(s.to_string());
                }
                if output.stderr.len() > 0 {
                    let s = String::from_utf8_lossy(&output.stderr);
                    let _ = &s.lines().map(|l| debug!("    stderr: {}", l)).collect::<Vec<_>>();
                    result.logs.push(s.to_string());
                }
                if !output.status.success(){
                    error!("Step \"{}\" returned execution failure! aborting next steps", s_name);
                    logs.push(format!("Step \"{}\" returned execution failure! aborting next steps", s_name));
                    result.success = false;
                    break;
                }
                step_counter = step_counter + 1;
            }
            if !result.success {
                break;
            }
        }
        e.job_results.push(result);
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