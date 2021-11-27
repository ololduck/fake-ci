use std::collections::HashMap;
use std::env::current_dir;
use std::process::{Command, Output};

use anyhow::Result;
use log::{debug, error};
use rand::Rng;

use crate::conf::FakeCIDockerBuild;
use crate::utils::trim_newline;

#[cfg(test)]
mod tests {
    use std::fs::{File, remove_file};
    use std::io::Write;
    use tempdir::TempDir;
    use crate::build_image;
    use crate::conf::FakeCIDockerBuild;
    use crate::utils::docker::docker_remove_image;
    use pretty_assertions::{assert_eq};
    use crate::utils::tests::with_dir;

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
                privileged: false
            };
            let image = build_image(&config).expect("Could not build image");
            assert_eq!(image, "fakeci-build-image-test");
            let _ = docker_remove_image(&image);
            let _ = remove_file("Dockerfile");
        });
    }
}

const DOCKER_NAME_CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz-_0123456789";

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

/// builds an image, returning the name of the newly built image
pub fn build_image(config: &FakeCIDockerBuild) -> Result<String> {
    debug!("build image called with {:#?}", config);
    let mut rng = rand::thread_rng();
    let rand_name = format!(
        "fakeci-{}",
        (0..12)
            .map(|_| {
                let idx = rng.gen_range(0, DOCKER_NAME_CHARSET.len());
                DOCKER_NAME_CHARSET[idx] as char
            })
            .collect::<String>()
    );
    let name = &config.name.as_ref().unwrap_or(&rand_name);
    let default_context = ".".to_string();
    let args = &[
        "build",
        &format!(
            "--file={}",
            &config.dockerfile.as_ref().unwrap_or(&"Dockerfile".to_string())
        ),
        "-t",
        name,
        &config.context.as_ref().unwrap_or(&default_context)
    ];
    debug!("Running docker {}", args.join(" "));
    let output = Command::new("docker")
        .args(args)
        .current_dir(config.context.as_ref().unwrap_or(&".".to_string()))
        .output()?;
    if !output.status.success() {
        error!("Error on docker build: {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::Error::msg(format!("Could not build docker image {}", args[3])));
    }

    Ok(name.to_string())
}

/// Removes the given image
pub fn docker_remove_image(image: &str) -> Result<()> {
    let args = &["rmi", image];
    debug!("Running docker {}", args.join(" "));
    let output = Command::new("docker").args(args).output()?;
    if !output.status.success() {
        return Err(anyhow::Error::msg("Could not remove docker image"));
    }
    Ok(())
}

/// Runs the given `command` in a container created from `image`.
/// ```rust
/// # use std::collections::HashMap;
/// # use std::process::Output;
/// # use fakeci::utils::docker::run_from_image;
/// # let _ = pretty_env_logger::try_init();
/// # use pretty_assertions::assert_eq;
/// let o: Output = run_from_image("busybox", "echo hi", &[], &HashMap::default(), true, false).expect("could not run docker :'(");
/// assert_eq!(o.status.success(), true);
/// assert_eq!(String::from_utf8_lossy(&o.stdout), "hi\n");
/// ```
pub fn run_from_image(
    image: &str,
    command: &str,
    volumes: &[String],
    env: &HashMap<String, String>,
    one_time: bool,
    privileged: bool
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
            .map(|v| format!("-v {}", v))
            .collect::<Vec<String>>(),
    );
    // yeah, we can't have a &String if the object is freed...
    let s_run = String::from("run");
    let args = {
        let mut args: Vec<&str> = Vec::new();
        args.push(&s_run);
        if one_time {
            args.push("--rm");
        }
        if privileged {
            args.push("--privileged");
        }
        args.push("--workdir=/code");
        args.extend(vols.iter().map(|v| v.as_str()));
        args.push(image);
        args.extend(command.split_whitespace());
        args
    };
    debug!("Running \"docker {}\"", &args.join(" "));
    let out = Command::new("docker").args(args).envs(env).output()?;
    debug!("docker execution over");
    Ok(out)
}
