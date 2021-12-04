use std::env;
use std::fs::File;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tempdir::TempDir;

use crate::conf::{FakeCIRepoConfig, Image};
use crate::utils::docker::{
    build_image, docker_remove_container, run_from_image, run_in_container,
};
use crate::utils::get_job_image_or_default;
use crate::utils::git::{get_commit, git_clone_with_branch_and_path, Commit};

pub mod conf;
pub mod notifs;
pub mod utils;

#[cfg(test)]
mod tests {
    use std::fs::remove_file;
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use crate::utils::tests::{deser_yaml, get_sample_resource_file, with_dir};
    use crate::{execute_config, launch};

    #[test]
    fn hello_world() {
        let _ = pretty_env_logger::try_init();
        let conf = "pipeline:
  - name: \"hello world\"
    image: busybox
    steps:
      - name: \"Create File\"
        exec:
          - \"touch hello_world\"";
        let config = serde_yaml::from_str(conf).unwrap();
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        with_dir(&p, || {
            assert!(execute_config(config).is_ok());
            let hello = p.join("hello_world");
            assert!(hello.is_file());
            remove_file(hello).expect("Could not remove file in test_hello_world");
        });
    }

    #[test]
    fn multiple_steps() -> anyhow::Result<()> {
        let _ = pretty_env_logger::try_init();
        let conf = deser_yaml(&get_sample_resource_file("job_container_reuse.yml")?)?;
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        with_dir(&p, || {
            let result = execute_config(conf);
            assert!(result.is_ok());
            let result = result.unwrap();
            for j in result.job_results {
                assert!(j.success);
                assert!(j.logs.contains(&"hi!\n".to_string()));
            }
        });
        Ok(())
    }

    #[test]
    #[ignore]
    fn dog_feeding() {
        let _ = pretty_env_logger::try_init();
        let r = launch(".", "main");
        assert!(r.is_ok());
        let r = r.unwrap();
        for job_result in r.job_results {
            assert_eq!(job_result.success, true);
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct JobResult {
    pub success: bool,
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub logs: Vec<String>,
}

impl JobResult {
    pub fn duration(&self) -> Duration {
        self.end_date - self.start_date
    }
}

impl Default for JobResult {
    fn default() -> Self {
        Self {
            success: false,
            name: "".to_string(),
            start_date: Utc::now(),
            end_date: Utc::now(),
            logs: vec![],
        }
    }
}

#[derive(Default, Serialize)]
pub struct ExecutionContext {
    pub branch: String,
    pub commit: Commit,
}

#[derive(Serialize)]
pub struct ExecutionResult {
    pub job_results: Vec<JobResult>,
    pub context: ExecutionContext,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            job_results: vec![],
            context: Default::default(),
            start_date: Utc::now(),
            end_date: Utc::now(),
        }
    }
}

#[allow(clippy::explicit_counter_loop)]
fn execute_config(conf: FakeCIRepoConfig) -> Result<ExecutionResult> {
    let mut e = ExecutionResult::default();
    for job in &conf.pipeline {
        info!("Running job \"{}\"", job.name);
        let mut logs: Vec<String> = Vec::new();
        let mut result = JobResult {
            success: true,
            start_date: Utc::now(),
            name: String::from(&job.name),
            ..Default::default()
        };
        let image = match get_job_image_or_default(job, &conf) {
            Ok(i) => i,
            Err(e) => {
                error!("Could not find image definition anywhere!: {}", e);
                return Err(e);
            }
        };
        let image_str = match image {
            Image::Existing(s) => s.clone(),
            Image::Build(i) => build_image(i)?,
            Image::ExistingFull(e) => e.name.clone(),
        };

        let mut volumes = Vec::new();
        if let Some(vols) = job.volumes.as_ref() {
            volumes.extend(vols.iter().map(String::from).collect::<Vec<String>>());
        }
        // first, create the container
        let cname = job.generate_container_name();
        let output = run_from_image(
            &image_str,
            &cname,
            "sh",
            &volumes,
            &job.env.clone().unwrap_or_default(),
            false,
            image.is_privileged(),
        )?;
        if !output.status.success() {
            error!("Failure to create container {}", cname);
            result
                .logs
                .push(format!("ERROR: Failure to create container {}", cname));
            result.success = false;
            e.job_results.push(result);
            break;
        }
        debug!("Successfully created container {}", cname);

        // then, run the steps
        for step in &job.steps {
            let mut step_counter = 0;
            let step_counter_as_str = step_counter.to_string();
            let s_name = step.name.as_ref().unwrap_or(&step_counter_as_str);
            info!(" Running step \"{}\"", s_name);
            result.logs.push(format!("--- Step {} ---", s_name));
            for e in &step.exec {
                info!("  - {}", e);
                let output = run_in_container(&cname, e)?;
                if !output.stdout.is_empty() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    let _ = &s
                        .lines()
                        .map(|l| debug!("    stdout: {}", l))
                        .collect::<Vec<_>>();
                    result.logs.push(s.to_string());
                }
                if !output.stderr.is_empty() {
                    let s = String::from_utf8_lossy(&output.stderr);
                    let _ = &s
                        .lines()
                        .map(|l| debug!("    stderr: {}", l))
                        .collect::<Vec<_>>();
                    result.logs.push(s.to_string());
                }
                if !output.status.success() {
                    error!(
                        "Step \"{}\" returned execution failure! aborting next steps",
                        s_name
                    );
                    logs.push(format!(
                        "Step \"{}\" returned execution failure! aborting next steps",
                        s_name
                    ));
                    result.success = false;
                    break;
                }
                step_counter += 1;
            }
            if !result.success {
                break;
            }
        }
        result.end_date = Utc::now();
        e.job_results.push(result);
        docker_remove_container(&cname)?;
    }
    e.end_date = Utc::now();
    Ok(e)
}

fn execute_from_file(path: &Path) -> Result<ExecutionResult> {
    debug!("Execute from file {}", path.display());
    let c = match serde_yaml::from_reader(File::open(path)?) {
        Ok(c) => c,
        Err(e) => {
            error!("Could not parse yaml config: {}", e);
            panic!();
        }
    };
    let r = execute_config(c)?;
    Ok(r)
}

pub fn launch(repo_url: &str, branch: &str) -> Result<ExecutionResult> {
    debug!("launch called with repo {}", repo_url);
    let root = TempDir::new("fakeci_execution")?;
    debug!("running in dir {}", root.path().display());
    git_clone_with_branch_and_path(repo_url, branch, root.path())?;
    let old_path = env::current_dir()?;
    env::set_current_dir(root.path())?;
    let mut r = execute_from_file(Path::new(".fakeci.yml"))?;
    r.context.branch = branch.to_string();
    r.context.commit = get_commit("HEAD")?;
    env::set_current_dir(old_path)?;
    Ok(r)
}
