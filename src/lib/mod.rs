use std::env;
use std::fs::File;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use log::{debug, error, info};
use serde::Serialize;
use tempdir::TempDir;

use crate::conf::{FakeCIRepoConfig, IMAGE};
use crate::utils::docker::{
    build_image, docker_remove_container, run_from_image, run_in_container,
};
use crate::utils::get_job_image_or_default;

pub mod conf;
pub mod utils;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::fs::remove_file;
    use std::path::PathBuf;

    use crate::utils::tests::{deserialize, get_sample_resource_file, with_dir};
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
        let conf = deserialize(&get_sample_resource_file("job_container_reuse.yml")?)?;
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
    pub logs: Vec<String>,
}

#[derive(Default, Serialize)]
pub struct ExecutionResult {
    pub job_results: Vec<JobResult>,
    pub artifacts: Vec<String>,
}

fn execute_config(conf: FakeCIRepoConfig) -> Result<ExecutionResult> {
    let mut e = ExecutionResult::default();
    for job in &conf.pipeline {
        info!("Running job \"{}\"", job.name);
        let mut logs: Vec<String> = Vec::new();
        let mut result = JobResult {
            success: true,
            ..Default::default()
        };
        let image = match get_job_image_or_default(&job, &conf) {
            Ok(i) => i,
            Err(e) => {
                error!("Could not find image definition anywhere!: {}", e);
                return Err(e);
            }
        };
        let image_str = match image {
            IMAGE::Existing(s) => s.clone(),
            IMAGE::Build(i) => build_image(i)?,
            IMAGE::ExistingFull(e) => e.name.clone(),
        };

        let mut volumes = Vec::new();
        if let Some(vols) = job.volumes.as_ref() {
            volumes.extend(
                vols.iter()
                    .map(|s| String::from(s))
                    .collect::<Vec<String>>(),
            );
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
            for e in &step.exec {
                info!("  - {}", e);
                let output = run_in_container(&cname, e)?;
                if output.stdout.len() > 0 {
                    let s = String::from_utf8_lossy(&output.stdout);
                    let _ = &s
                        .lines()
                        .map(|l| debug!("    stdout: {}", l))
                        .collect::<Vec<_>>();
                    result.logs.push(s.to_string());
                }
                if output.stderr.len() > 0 {
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
                step_counter = step_counter + 1;
            }
            if !result.success {
                break;
            }
        }
        e.job_results.push(result);
        docker_remove_container(&cname)?;
    }
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

pub fn launch(repo_url: &str) -> Result<ExecutionResult> {
    debug!("launch called with repo {}", repo_url);
    let root = TempDir::new("fakeci_execution")?;
    debug!("running in dir {}", root.path().display());
    let output = Command::new("git")
        .args([
            "clone",
            repo_url,
            root.path()
                .to_str()
                .expect("Could not convert from tmpdir to str"),
        ])
        .output()?;
    if !output.status.success() {
        error!("Could not clone repo!!!!!!!S");
        panic!();
    }
    let old_path = env::current_dir()?;
    env::set_current_dir(root.path())?;
    let r = execute_from_file(Path::new(".fakeci.yml"))?;
    //
    env::set_current_dir(old_path)?;
    Ok(r)
}
