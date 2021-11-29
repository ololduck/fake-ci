use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// all utility functions git-related
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use log::{debug, error};
use regex::Regex;

lazy_static! {
    static ref REF_PATTERN: Regex = Regex::new(r"([0-9a-fA-F]+)[ \t]+refs/heads/([a-z/\-_]+)")
        .expect("could not compile pattern");
}

#[cfg(test)]
mod tests {
    use log::trace;
    use pretty_assertions::assert_eq;
    use pretty_env_logger::try_init;

    use crate::utils::git::{fetch, REF_PATTERN};

    #[test]
    fn test_ref_pattern() {
        let s = "17af6fe1acfcf453025c8f221fdcf8842acbb38b        refs/heads/main";
        let cap = REF_PATTERN.captures(s).expect("could not match pattern");
        trace!("capture: {:#?}", cap);
        assert_eq!(
            cap[1].to_string(),
            "17af6fe1acfcf453025c8f221fdcf8842acbb38b"
        );
        assert_eq!(cap[2].to_string(), "main");
    }

    #[test]
    fn test_fetch() {
        let _ = try_init();
        let res = fetch("https://github.com/paulollivier/fake-ci").expect("could not list remote");
        trace!("res: {:#?}", res);
        assert!(res.contains_key("main"));
        assert!(res.get("main").unwrap_or(&"".to_string()).len() > 0);
    }
}

/// Fetches all the remotes in repo
/// ```
/// # use fakeci::utils::git::fetch;
/// # use pretty_env_logger::try_init;
/// # use log::trace;
/// # let _ = try_init();
/// let res = fetch("https://github.com/paulollivier/fake-ci").expect("could not list remote");
/// # trace!("{:#?}", res);
/// assert!(res.contains_key("main"));
/// assert!(res.get("main").unwrap_or(&"".to_string()).len() > 0);
/// ```
pub fn fetch(uri: &str) -> Result<HashMap<String, String>> {
    debug!("Running git ls-remote --heads {}", uri);
    let o = Command::new("git")
        .arg("ls-remote")
        .arg("--heads")
        .arg(uri)
        .output()?;
    if !o.status.success() {
        error!("failed to run git ls-remote --heads {}", uri);
        return Err(anyhow!("failed to run git ls-remote --heads {}", uri));
    }

    let i: HashMap<String, String> = String::from_utf8(o.stdout)?
        .lines()
        .filter_map(|line| REF_PATTERN.captures(line))
        .map(|capture| (capture[2].to_string(), capture[1].to_string()))
        .collect();
    Ok(HashMap::from_iter(i))
}

pub fn git_clone_with_branch_and_path(repo_url: &str, branch: &str, to: &Path) -> Result<()> {
    let output = Command::new("git")
        .args([
            "clone",
            repo_url,
            to.to_str().expect("Could not convert from path to str"),
        ])
        .output()?;
    if !output.status.success() {
        error!("could not git clone {}!", repo_url);
        return Err(anyhow!("Could not git clone {}!", repo_url));
    }
    let output = Command::new("git")
        .args(&[
            &format!("--git-dir={}/.git", to.display()),
            &format!("--work-tree={}", to.display()),
            "checkout",
            branch,
        ])
        .output()?;
    if !output.status.success() {
        error!("Could not checkout {}", branch);
        return Err(anyhow!("Could not checkout {}!", branch));
    }
    Ok(())
}
