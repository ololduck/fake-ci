use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tempdir::TempDir;

use crate::conf::{FakeCIRepoConfig, Image};
use crate::utils::docker::{
    build_image, docker_remove_container, run_from_image, run_in_container,
};
use crate::utils::get_job_image_or_default;
use crate::utils::git::{get_commit, git_clone_with_branch_and_path, Commit};

/// All that is configuration-related. Structs related to file deserialization.
pub mod conf;
/// All outbound communications with the outside world
pub mod notifications;
/// Some utility functions, such as git or docker runs
pub mod utils;

#[cfg(test)]
mod tests {
    use std::fs::{remove_file, File};
    use std::io::{Read, Write};
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;
    use tempdir::TempDir;

    use crate::utils::tests::{deser_yaml, get_sample_resource_file, with_dir};
    use crate::{execute_config, execute_from_file, Env, FakeCIRepoConfig, LaunchOptions};

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
            assert!(execute_config(
                config,
                &LaunchOptions {
                    repo_name: "fake-ci tests".to_string(),
                    repo_url: ".".to_string(),
                    ..Default::default()
                }
            )
            .is_ok());
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
            let result = execute_config(
                conf,
                &LaunchOptions {
                    repo_name: "fake-ci tests".to_string(),
                    repo_url: ".".to_string(),
                    ..Default::default()
                },
            );
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
    fn secrets() {
        let _ = pretty_env_logger::try_init();
        let c = get_sample_resource_file("secrets.yml").expect("not found");
        let conf: FakeCIRepoConfig = serde_yaml::from_str(&c).expect("Could not parse yaml");
        let opts = LaunchOptions {
            repo_name: "fake-ci tests".to_string(),
            secrets: {
                let mut s = Env::new();
                s.insert("MY_SECRET".to_string(), "shh!".to_string());
                s
            },
            ..Default::default()
        };
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        with_dir(&p, || {
            let res = execute_config(conf, &opts);
            assert!(res.is_ok());
            let res = res.unwrap();
            assert_eq!(res.job_results.len(), 1);
            let j0 = res.job_results.get(0).unwrap();
            assert_eq!(
                j0.logs.contains(opts.secrets.get("MY_SECRET").unwrap()),
                false
            );
            let mut f = File::open("secrets.txt").unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            let _ = remove_file("secrets.txt");
            assert_eq!(&s, opts.secrets.get("MY_SECRET").unwrap());
        });
    }
    #[test]
    fn undefined_secret() {
        let _ = pretty_env_logger::try_init();
        let c = get_sample_resource_file("secrets_undefined.yml").expect("not found");
        let conf: FakeCIRepoConfig = serde_yaml::from_str(&c).expect("Could not parse yaml");
        let opts = LaunchOptions {
            repo_name: "fake-ci tests".to_string(),
            ..Default::default()
        };
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        with_dir(&p, || {
            let res = execute_config(conf, &opts);
            assert!(res.is_ok());
            let mut f = File::open("secrets.txt").unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            let _ = remove_file("secrets.txt");
            assert_eq!(&s, "");
        });
    }
    #[test]
    fn malformed_config() {
        let root = TempDir::new("malformed-config").expect("could not create tmp dir");
        let s = "malformed ymal";
        let p = root.path().join(".fakeci.yml");
        let mut f = File::create(&p).expect("could not create file");
        assert!(f.write_all(s.as_ref()).is_ok());
        let r = execute_from_file(&p, &LaunchOptions::default());
        assert!(r.is_err());
    }
}

#[derive(Deserialize, Serialize, Debug)]
/// The result of a single job.
pub struct JobResult {
    /// If all the steps returned 0.
    pub success: bool,
    /// Name of the job.
    pub name: String,
    /// When this job started.
    pub start_date: DateTime<Utc>,
    /// When this job ended.
    pub end_date: DateTime<Utc>,
    /// An array of strings, each a line of the steps' `stdout`
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

#[derive(Default, Serialize, Debug)]
/// The context in which the job executed
pub struct ExecutionContext {
    /// an arbitrary name, copied from `LaunchOptions`
    pub repo_name: String,
    /// the repository URL
    pub repo_url: String,
    /// the "branch" (read: git ref) used to run.
    pub branch: String,
    /// Some details regarding the commit designed by the branch.
    pub commit: Commit,
}

#[derive(Serialize, Debug)]
/// The result of executing all the jobs defined in the repository, with some context added.
pub struct ExecutionResult {
    /// An array of `JobResult`
    pub job_results: Vec<JobResult>,
    /// The context in which the job has executed
    pub context: ExecutionContext,
    /// When the job started
    pub start_date: DateTime<Utc>,
    /// When the job ended
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
fn execute_config(conf: FakeCIRepoConfig, opts: &LaunchOptions) -> Result<ExecutionResult> {
    let mut e = ExecutionResult {
        job_results: vec![],
        context: ExecutionContext {
            repo_name: opts.repo_name.to_string(),
            repo_url: opts.repo_url.to_string(),
            branch: opts.branch.to_string(),
            commit: get_commit("HEAD")?,
        },
        start_date: Utc::now(),
        ..Default::default()
    };
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

        let volumes = job
            .volumes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        // first, create the container
        let cname = job.generate_container_name();
        // Create the env
        let mut env = Env::new();
        if let Some(default_conf) = &conf.default {
            env.extend(default_conf.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        env.extend(job.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        env.extend(opts.environment.iter().map(|(k, v)| (k.clone(), v.clone())));
        env.extend({
            let mut secrets = Env::new();
            for secret in job.secrets.iter() {
                if let Some(v) = opts.secrets.get(secret) {
                    secrets.insert(secret.to_string(), v.to_string());
                } else {
                    return Err(anyhow!(
                        "Could not find secret {} in the executor's secrets!",
                        secret
                    ));
                }
            }
            secrets
        });
        // Then, run the stuff
        let output = run_from_image(
            &image_str,
            &cname,
            "sh",
            &volumes,
            &env,
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

fn execute_from_file(path: &Path, opts: &LaunchOptions) -> Result<ExecutionResult> {
    debug!("Execute from file {}", path.display());
    let c = match serde_yaml::from_reader(File::open(path)?) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Could not parse yaml config for branch {} in repo {}: {}",
                opts.branch, opts.repo_name, e
            );
            return Err(anyhow!(e));
        }
    };
    let r = execute_config(c, opts)?;
    Ok(r)
}
pub type Env = HashMap<String, String>;

#[derive(Default)]
/// Represents a test launch configuration. This is passed by the caller, probably an interface to the outside world
pub struct LaunchOptions {
    /// A name. Will be used in notifiers.
    pub repo_name: String,
    /// URL of the repository
    pub repo_url: String,
    /// branch to checkout
    pub branch: String,
    /// A HashMap of _secrets_, stuff that shouldn't be committed.
    pub secrets: Env,
    /// A HashMap of env values. Will be added to this launch's envvars
    pub environment: Env,
}

/// Launches the CI job for the repository
pub fn launch(opts: LaunchOptions) -> Result<ExecutionResult> {
    debug!("launch called with repo {}", opts.repo_url);
    let root = TempDir::new("fakeci_execution")?;
    debug!("running in dir {}", root.path().display());
    git_clone_with_branch_and_path(&opts.repo_url, &opts.branch, root.path())?;
    let old_path = env::current_dir()?;
    env::set_current_dir(root.path())?;
    let p = Path::new(".fakeci.yml");
    let r = execute_from_file(p, &opts)?;
    env::set_current_dir(old_path)?;
    Ok(r)
}
