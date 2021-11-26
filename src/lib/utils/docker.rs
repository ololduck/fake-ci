use crate::utils::trim_newline;
use anyhow::Result;
use log::debug;
use std::collections::HashMap;
use std::env::current_dir;
use std::process::{Command, Output};

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

/// Runs the given `command` in a container created from `image`.
/// ```rust
/// use std::collections::HashMap;
/// use std::process::Output;
/// use fakeci::utils::docker::run_in_container;
///
/// let o: Output = run_in_container("busybox", "echo hi", &[], &HashMap::default(), true).expect("could not run docker :'(");
/// assert_eq!(o.status.success(), true);
/// assert_eq!(String::from_utf8_lossy(&o.stdout), "hi\n");
/// ```
pub fn run_in_container(
    image: &str,
    command: &str,
    volumes: &[String],
    env: &HashMap<String, String>,
    one_time: bool,
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
