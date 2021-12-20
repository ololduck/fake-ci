use std::env::current_dir;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use anyhow::{anyhow, Result};
use log::{debug, error};
use rand::Rng;

use crate::conf::FakeCIDockerBuild;
use crate::utils::trim_newline;
use crate::Env;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env::current_dir;
    use std::fs::{remove_file, File};
    use std::io::Write;

    use pretty_assertions::{assert_eq, assert_ne};
    use tempdir::TempDir;

    use crate::conf::FakeCIDockerBuild;
    use crate::utils::docker::{docker_remove_image, rng_docker_chars};
    use crate::utils::tests::with_dir;
    use crate::{build_image, docker_remove_container, run_from_image, run_in_container, Env};

    #[test]
    fn docker_build() {
        let _ = pretty_env_logger::try_init();
        let tmp_dir = TempDir::new("dbuild").expect("could not create temp dir");
        with_dir(tmp_dir.path(), || {
            let mut f = File::create("Dockerfile").expect("could not create file");
            let _ = f.write("FROM busybox\nRUN echo 'hello world'\n".as_ref());
            let config = FakeCIDockerBuild {
                dockerfile: Some("Dockerfile".to_string()),
                context: None,
                build_args: None,
                name: Some("fakeci-build-image-test".to_string()),
                privileged: false,
            };
            let image = build_image(&config).expect("Could not build image");
            assert_eq!(image, "fakeci-build-image-test");
            let _ = docker_remove_image(&image);
            let _ = remove_file("Dockerfile");
        });
    }

    #[test]
    fn run_with_env() {
        let _ = pretty_env_logger::try_init();
        let tmp_dir = TempDir::new("dbuild").expect("could not create temp dir");
        with_dir(tmp_dir.path(), || {
            println!("current_dir: {}", current_dir().unwrap().display());
            let mut env = HashMap::new();
            env.insert("TEST_VAL".to_string(), "duck".to_string());
            let cname = format!("fake-ci-tests-{}", rng_docker_chars(4));
            let o = run_from_image("busybox", &cname, "sh", &vec![], &env, false, false);
            assert!(o.is_ok());
            let o = run_in_container(&cname, "echo val=$TEST_VAL");
            assert!(o.is_ok());
            let o = o.unwrap();
            assert!(o.status.success());
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            let _ = docker_remove_container(&cname);
            assert_ne!(s, "val=\n");
            assert_eq!(s, "val=duck\n");
        });
    }

    #[test]
    fn run_with_volumes() {
        let _ = pretty_env_logger::try_init();
        let tmp_dir = TempDir::new("dbuild").expect("could not create temp dir");
        with_dir(tmp_dir.path(), || {
            println!("current_dir: {}", current_dir().unwrap().display());
            let vols = vec!["/var/run/docker.sock:/var/run/docker.sock".to_string()];
            let cname = format!("fake-ci-tests-{}", rng_docker_chars(4));
            let o = run_from_image("busybox", &cname, "sh", &vols, &Env::new(), false, false);
            assert!(o.is_ok());
            let o = o.unwrap();
            assert!(o.status.success());
        });
    }
}

pub(crate) const DOCKER_NAME_CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz-_0123456789";

#[allow(dead_code)]
pub(crate) fn get_pwd_from_image(image: &str) -> Result<String> {
    debug!("Getting pwd with command docker run --rm {} pwd", image);
    let output = Command::new("docker")
        .args(["run", "--rm", image, "pwd"])
        .output()?;
    if !output.status.success() {
        panic!("Could not get default dir of image {}", image);
    }
    let mut s = String::from_utf8_lossy(&output.stdout).to_string();
    trim_newline(&mut s);
    debug!("Result pwd: {:?}", s);
    Ok(s)
}

pub(crate) fn cwd() -> Result<String> {
    Ok(format!("{}", current_dir()?.display()))
}

fn docker_cmd(args: &[&str], current_dir: &str) -> Result<Output> {
    debug!("Running in {}: docker {}", current_dir, args.join(" "));
    Ok(Command::new("docker")
        .args(args)
        .current_dir(current_dir)
        .output()?)
}

/// builds an image, returning the name of the newly built image
pub fn build_image(config: &FakeCIDockerBuild) -> Result<String> {
    debug!("build image called with {:?}", config);
    let rand_name = rng_docker_chars(12);
    let name = &config.name.as_ref().unwrap_or(&rand_name);
    let default_context = ".".to_string();
    let args = &[
        "build",
        &format!(
            "--file={}",
            &config
                .dockerfile
                .as_ref()
                .unwrap_or(&"Dockerfile".to_string())
        ),
        "-t",
        name,
        config.context.as_ref().unwrap_or(&default_context),
    ];
    let output = docker_cmd(args, config.context.as_ref().unwrap_or(&".".to_string()))?;
    if !output.status.success() {
        error!(
            "Error on docker build: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(anyhow!("Could not build docker image {}", args[3]));
    }

    Ok(name.to_string())
}

pub(crate) fn rng_docker_chars(n: u8) -> String {
    let mut rng = rand::thread_rng();
    let rand_name = format!(
        "fakeci-{}",
        (0..n)
            .map(|_| {
                let idx = rng.gen_range(0..DOCKER_NAME_CHARSET.len());
                DOCKER_NAME_CHARSET[idx] as char
            })
            .collect::<String>()
    );
    rand_name
}

/// Removes the given image
pub fn docker_remove_image(image: &str) -> Result<()> {
    let args = &["rmi", image];
    let output = docker_cmd(args, &cwd()?)?;
    if !output.status.success() {
        return Err(anyhow!("Could not remove docker image"));
    }
    Ok(())
}

pub fn docker_remove_container(container: &str) -> Result<()> {
    let args = &["rm", container];
    let output = docker_cmd(args, &cwd()?)?;
    if !output.status.success() {
        return Err(anyhow!("Could not remove docker container {}", container));
    }
    Ok(())
}

/// Runs the given command in the given container, then returns the output.
/// ```rust,no_run
/// # use std::collections::HashMap;
/// use fakeci::utils::docker::{docker_remove_container, run_from_image, run_in_container};
/// let image = "ubuntu";
/// let cname = "fakeci-container-reuse-doctest";
/// let commands = vec!["ls", "echo hello world"];
/// let _ = run_from_image(image, cname, "bash", &[], &HashMap::default(), false, false);
/// for cmd in commands {
///     let o = run_in_container(cname, cmd);
///     assert!(o.is_ok());
///     let status = o.unwrap().status;
///     assert!(status.success());
/// }
/// let _ = docker_remove_container(cname);
/// ```
pub fn run_in_container(container: &str, command: &str) -> Result<Output> {
    let args = &["start", "-ai", container];
    debug!("Running docker {}", &args.join(" "));
    let mut process = Command::new("docker")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let c_stdin = process.stdin.as_mut().unwrap();
    debug!("piping \"{}\" to {}", command, container);
    c_stdin.write_all(command.as_bytes())?;
    Ok(process.wait_with_output()?)
}

/// Runs the given `command` in a container created from `image`.
/// ```rust,no_run
/// # use std::collections::HashMap;
/// # use std::process::Output;
/// # use fakeci::utils::docker::run_from_image;
/// # let _ = pretty_env_logger::try_init();
/// # use pretty_assertions::assert_eq;
/// let output = run_from_image("busybox", "fake-ci-doctest","sh", &[], &HashMap::default(), true, false).expect("could not run docker :'(");
/// assert_eq!(output.status.success(), true);
/// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
/// ```
pub fn run_from_image(
    image: &str,
    container_name: &str,
    command: &str,
    volumes: &[String],
    env: &Env,
    one_time: bool,
    privileged: bool,
) -> Result<Output> {
    let mut vols = vec![format!(
        "--volume={}:{}",
        current_dir()?
            .to_str()
            .expect("could not convert current dir to str"),
        "/code"
    )];
    vols.extend(
        volumes
            .iter()
            .map(|v| format!("--volume={}", v))
            .collect::<Vec<String>>(),
    );
    // yeah, we can't have a &String if the object is freed...
    let s_run = String::from("run");
    let cname = format!("--name={}", container_name);
    #[allow(clippy::into_iter_on_ref)]
    let env_args = env
        .into_iter()
        .flat_map(|(k, v)| {
            let v = vec!["-e".to_string(), format!("{}={}", k, v)];
            v.into_iter()
        })
        .collect::<Vec<String>>();
    let args = {
        let mut args: Vec<&str> = vec![&s_run, "-i"];
        if one_time {
            args.push("--rm");
        }
        if privileged {
            args.push("--privileged");
        }
        args.push(&cname);
        args.push("--workdir=/code");
        args.extend(vols.iter().map(|v| v.as_str()));
        args.extend(env_args.iter().map(|s| s.as_str()));
        args.push("--pull=always");
        args.push(image);
        args.extend(command.split_whitespace());
        args
    };
    debug!("Running docker {}", &args.join(" "));
    let mut proc = Command::new("docker")
        .args(args)
        .envs(env)
        .stdin(Stdio::piped())
        .spawn()?;
    {
        let stdin = proc.stdin.as_mut().unwrap();
        debug!("writing exit to stdin…");
        stdin.write_all(b"exit")?;
    }
    debug!("waiting for docker run completion…");
    let out = proc.wait_with_output()?;
    debug!("docker execution over");
    Ok(out)
}
