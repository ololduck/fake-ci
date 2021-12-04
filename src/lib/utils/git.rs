use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Command;

/// all utility functions git-related
use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use lazy_static::lazy_static;
use log::{debug, error};
use regex::Regex;
use serde::Serialize;

lazy_static! {
    static ref REF_PATTERN: Regex = Regex::new(r"([0-9a-fA-F]+)[ \t]+refs/heads/([a-z/\-_]+)")
        .expect("could not compile pattern");
    static ref COMMIT_PERSON_PATTERN: Regex =
        Regex::new(r"([A-Za-z\-_ ]+) <([a-z0-9_\-\.\+]+@[a-z0-9\.\-_]+)> ([0-9]+ (\+|\-)[0-9]{4})")
            .expect("could not compile pattern");
}

#[cfg(test)]
mod tests {
    use log::trace;
    use pretty_assertions::assert_eq;
    use pretty_env_logger::try_init;

    use crate::utils::git::{fetch, parse_raw_commit, REF_PATTERN};

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

    #[test]
    fn test_commit_parsing() {
        let s = "commit 970683e1d18cf8229795fc8346ef6f66c0e8b2b0
tree 0c7f2dba4403ebcfc576cb7fb0e9c7273b12eab9
parent b4ff70f0ac937af2871ad020c6eef8a2c925a392
author Paul Ollivier <contact@paulollivier.fr> 1638209781 +0100
committer Paul Ollivier <contact@paulollivier.fr> 1638209781 +0100

    Add notification interface";
        let c = parse_raw_commit(s);
        assert!(c.is_ok());
        let c = c.unwrap();
        assert_eq!(c.author.name, "Paul Ollivier");
        assert_eq!(c.author.email, "contact@paulollivier.fr".to_string());
        assert_eq!(
            format!("{}", c.committer),
            "Paul Ollivier <contact@paulollivier.fr> 2021-11-29T18:16:21+00:00".to_string()
        );
        assert_eq!(c.hash, "970683e1d18cf8229795fc8346ef6f66c0e8b2b0");
        assert_eq!(c.message, "Add notification interface");
        assert_eq!(c.parents.len(), 1);
        assert_eq!(c.parents[0], "b4ff70f0ac937af2871ad020c6eef8a2c925a392");
    }

    #[test]
    fn test_complex_commit_parsing() {
        let s = "commit b4ff70f0ac937af2871ad020c6eef8a2c925a392
tree b8f59264d9f43b05121baa999fd27121cf1f764c
parent 17af6fe1acfcf453025c8f221fdcf8842acbb38b
parent 6aa86ed20f8444191330ba5f6c1ee27a5a8edd3f
author Paul Ollivier <contact@paulollivier.fr> 1638209074 +0100
committer GitHub <noreply@github.com> 1638209074 +0100
gpgsig -----BEGIN PGP SIGNATURE-----

 wsBcBAABCAAQBQJhpRYyCRBK7hj4Ov3rIwAATiMIAHQ21Ve+8ecDID+zG/xsXHKo
 Owe3kz+iBbB+837Nxcswu6qdK/W/KO4WwEzlrjc9Yf89IwWZCya1wI/vJnmlLnqo
 6LTZJMRyaJZSYCrW8DsHfrjK7mtyBSN0Se0mDqieVVy9WK/hVhJphe1m9cCtaocG
 /9TTJ86KwAfveiAuKptKSd8gvhlp1XdgSUtVK7yXQ07/IrFLPO+q9vwej5Xh0/L5
 FcmpoH7xjVPcq8XOTf0/22CbEuu6ZheAmkoR35886q/gXLnT3VdSWPoPyUztY/cT
 RaNDI+A/e/atyUv5F2eriv/m8xzvktk9X+dqB+4fgxgYlGcFH2uO6cK7CuYuOPE=
 =Z5N1
 -----END PGP SIGNATURE-----


    Merge pull request #12 from paulollivier/repository-watching

    Add loop-based repository watching";
        let c = parse_raw_commit(s);
        assert!(c.is_ok());
        let c = c.unwrap();
        assert_eq!(c.hash, "b4ff70f0ac937af2871ad020c6eef8a2c925a392");
        assert_eq!(c.tree, "b8f59264d9f43b05121baa999fd27121cf1f764c");
        assert_eq!(c.parents.len(), 2);
        assert_eq!(
            c.message,
            "Merge pull request #12 from paulollivier/repository-watching
Add loop-based repository watching"
        );
    }
}

#[derive(Debug, Serialize)]
pub struct CommitPerson {
    pub name: String,
    pub email: String,
    pub date: DateTime<Utc>,
}

impl From<&str> for CommitPerson {
    fn from(s: &str) -> Self {
        let matches = COMMIT_PERSON_PATTERN.captures(s);
        if let Some(matches) = matches {
            let dt = DateTime::parse_from_str(&matches[3].to_string(), "%s %z");
            if dt.is_err() {
                return CommitPerson::default();
            }
            let dt = dt.unwrap();
            let dt = dt.with_timezone(&Utc);
            return CommitPerson {
                name: matches[1].to_string(),
                email: matches[2].to_string(),
                date: dt,
            };
        }
        CommitPerson::default()
    }
}

impl Display for CommitPerson {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = String::from(&self.name);
        s.push_str(&format!(" <{}>", &self.email));
        s.push(' ');
        s.push_str(&self.date.to_rfc3339());
        f.write_str(&s)
    }
}

impl Default for CommitPerson {
    fn default() -> Self {
        CommitPerson {
            name: "".to_string(),
            email: "".to_string(),
            date: Utc::now(),
        }
    }
}

#[derive(Serialize)]
pub struct Commit {
    pub hash: String,
    pub author: CommitPerson,
    pub committer: CommitPerson,
    pub message: String,
    pub tree: String,
    pub parents: Vec<String>,
}

impl Default for Commit {
    fn default() -> Self {
        Commit {
            hash: "".to_string(),
            author: Default::default(),
            committer: Default::default(),
            message: "".to_string(),
            tree: "".to_string(),
            parents: vec![],
        }
    }
}

pub(crate) fn parse_raw_commit(raw: &str) -> Result<Commit> {
    let mut c = Commit::default();
    let mut has_found_git_msg = false;
    let mut has_found_gpg_sig = false;
    for line in String::from(raw).lines() {
        match line.starts_with("    ") {
            true => {
                //then its a message commit
                has_found_git_msg = true;
                let line = line.strip_prefix("    ");
                match line {
                    Some(s) => c.message.push_str(s),
                    None => {}
                }
            }
            false => {
                if line.is_empty() && !has_found_git_msg {
                    continue;
                } else if has_found_git_msg {
                    c.message.push_str("\n");
                    continue;
                } else if has_found_gpg_sig && line.starts_with(" ") {
                    continue;
                }
                let tokens = line.split_whitespace().collect::<Vec<&str>>();
                if tokens.len() < 2 {
                    return Err(anyhow!("weeeeeeird"));
                }
                match tokens[0] {
                    "commit" => c.hash = tokens[1].to_string(),
                    "tree" => c.tree = tokens[1].to_string(),
                    "parent" => c.parents.push(tokens[1].to_string()),
                    "author" => {
                        let mut iter = tokens.iter();
                        let _author = iter.next();
                        let vs: Vec<String> = iter.map(|s| s.to_string()).collect();
                        c.author = CommitPerson::from(vs.join(" ").as_str())
                    }
                    "committer" => {
                        let mut iter = tokens.iter();
                        let _author = iter.next();
                        let vs: Vec<String> = iter.map(|s| s.to_string()).collect();
                        c.committer = CommitPerson::from(vs.join(" ").as_str())
                    }
                    "gpgsig" => {
                        has_found_gpg_sig = true;
                    }
                    _ => {}
                };
            }
        };
    }
    Ok(c)
}

pub fn get_commit(reference: &str) -> Result<Commit> {
    let out = Command::new("git")
        .args(&["log", "-n", "1", "--format=raw", reference])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("Could not read commit {}", reference));
    }
    Ok(parse_raw_commit(&String::from_utf8_lossy(&out.stdout))?)
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
