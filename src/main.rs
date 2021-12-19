use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::{App, Arg, SubCommand};
use log::{debug, error, info, trace, warn, LevelFilter};
use serde::{Deserialize, Serialize};

use fakeci::notifications::Notifier;
use fakeci::utils::cache_dir;
use fakeci::utils::git::fetch;
use fakeci::{launch, Env, LaunchOptions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use crate::FakeCIBinaryConfig;
    use anyhow::Result;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;
    fn get_sample_resource_file(p: &str) -> Result<String> {
        let mut s = String::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let file_path = root.join("resources/tests").join(p);
        let mut f = File::open(file_path)?;
        let _count = f.read_to_string(&mut s);
        Ok(s)
    }
    #[test]
    fn notifier_config() {
        let c = get_sample_resource_file("notifiers.yml").expect("not found");
        let conf: FakeCIBinaryConfig = serde_yaml::from_str(&c).expect("Could not parse yaml");
        assert_eq!(conf.repositories.len(), 1);
        let _: () = conf
            .repositories
            .iter()
            .map(|repo| {
                assert_eq!(repo.notifiers.len(), 1);
            })
            .collect();
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(untagged)]
pub enum BranchesSpec {
    Single(String),
    Multiple(Vec<String>),
}
impl Default for BranchesSpec {
    fn default() -> Self {
        BranchesSpec::Single("*".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FakeCIBinaryRepositoryConfig {
    pub name: String,
    pub uri: String,
    pub branches: BranchesSpec,
    #[serde(default)]
    pub notifiers: Vec<Notifier>,
    #[serde(default)]
    pub secrets: Env,
    #[serde(default)]
    pub environment: Env,
    #[serde(skip, default)]
    pub refs: HashMap<String, String>,
    #[serde(skip, default)]
    pub br_regexps: Vec<glob::Pattern>,
}

impl FakeCIBinaryRepositoryConfig {
    // horribly inefficient function.
    // Hopefully we won't meet a repo with millions of branches.
    pub fn update_branches(&mut self) -> Result<HashMap<String, String>> {
        let mut diff = HashMap::new();
        let r = fetch(&self.uri)?;
        let deleted: Vec<String> = self
            .refs
            .keys()
            .filter(|k| !r.contains_key(*k))
            .map(|k| k.to_string())
            .collect();
        for d in &deleted {
            self.refs.remove(d);
        }
        let mut changed: HashMap<String, String> = HashMap::from_iter(
            r.iter()
                .filter(|(k, _)| !self.refs.contains_key(*k))
                .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        diff.extend(changed.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        for (k, v) in self.refs.iter() {
            if r.contains_key(k) && r.get(k).unwrap() != v {
                changed.insert(k.to_string(), r.get(k).unwrap().to_string());
                diff.insert(k.to_string(), r.get(k).unwrap().to_string());
            }
        }
        self.refs.extend(changed);
        Ok(diff)
    }

    pub fn init(&mut self) {
        let v = match &self.branches {
            BranchesSpec::Single(s) => {
                trace!("Compiling branch pattern {}", s);
                vec![glob::Pattern::new(s)
                    .unwrap_or_else(|_| panic!("could not compile regex {}", s))]
            }
            BranchesSpec::Multiple(v) => v
                .iter()
                .map(|s| {
                    trace!("Compiling branch pattern {}", s);
                    glob::Pattern::new(s)
                        .unwrap_or_else(|_| panic!("could not compile regex {}", s))
                })
                .collect(),
        };
        self.br_regexps = v;
        // find cache dir
        let cache = cache_dir();
        // read cache dir
        let mut s = String::new();
        let fname = cache.join(format!("{}.yml", self.name));
        let mut f = match File::open(&fname) {
            Ok(f) => f,
            Err(_e) => {
                warn!(
                    "Could not open file {} for persisted branch info",
                    fname.display()
                );
                return;
            }
        };
        let _ = f.read_to_string(&mut s);
        let refs: HashMap<String, String> = match serde_yaml::from_str(&s) {
            Ok(h) => h,
            Err(_) => {
                error!("could not deserialize file cache content, using fresh values");
                return;
            }
        };
        self.refs.extend(refs);
    }

    pub fn persist(&self) -> Result<()> {
        trace!("persist()");
        // find cache dir
        let cache = cache_dir();
        trace!("cache: {}", cache.display());
        create_dir_all(&cache)?;
        let mut f = File::create(cache.join(format!("{}.yml", self.name)))?;
        // write to cache dir
        let _ = f.write_all(serde_yaml::to_string(&self.refs)?.as_ref());
        debug!("Finished persisting branch values to disk");
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Config for the binary
/// ```
/// use fakeci::conf::FakeCIBinaryConfig;
/// let s: &str = "repositories:
///   - name: blabla
///     uri: https://github.com/paulollivier/fake-ci
///     branches: \"*\"";
/// let c: FakeCIBinaryConfig = serde_yaml::from_str(s).expect("invalid yaml");
/// assert_eq!(c.watch_interval, 300);
/// assert_eq!(c.repositories.len(), 1);
/// ```
pub struct FakeCIBinaryConfig {
    #[serde(default = "watch_interval_default")]
    pub watch_interval: u32,
    pub repositories: Vec<FakeCIBinaryRepositoryConfig>,
}

fn watch_interval_default() -> u32 {
    300
}

fn main() -> Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(LevelFilter::Trace)
        .init();
    let matches = App::new("fake-ci")
        .version(VERSION)
        .author("Paul O.")
        .about("A CI system written in rust")
        .arg(Arg::with_name("config").short("c").long("config").value_name("FILE").help("Sets a config file").takes_value(true).default_value("fake-ci.toml"))
        .subcommand(SubCommand::with_name("watch").about("Runs FakeCI in pulling mode; it will watch predefined repositories and attempt to pull them"))
        .get_matches();
    let mut config = read_fakeci_config_file(matches.value_of("config").unwrap())?;
    debug!("config: {:#?}", config);
    if let Some(_matches) = matches.subcommand_matches("watch") {
        debug!("found subcommand watch");
        let _ = watch(&mut config);
    }
    Ok(())
}

fn watch(config: &mut FakeCIBinaryConfig) -> Result<()> {
    debug!("watch() called with config {:#?}", config);
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    let wait_period = Duration::from_secs(config.watch_interval as u64);
    for r in config.repositories.iter_mut() {
        debug!("updating repo {}", r.name);
        r.init();
    }
    while !term.load(Ordering::Relaxed) {
        for repo in config.repositories.iter_mut() {
            debug!("Checking repo {}", repo.name);
            trace!("repo before update: {:#?}", repo);
            // fetch and see if there's changes, and on which branches
            let changes = repo.update_branches()?;
            trace!("repo after update: {:#?}", repo);
            info!("found changes: {:?}", changes);
            // if there's changes, execute the CI
            if changes.is_empty() {
                continue;
            }
            for branch in changes.keys().filter(|k| {
                repo.br_regexps.iter().any(|r| {
                    trace!("pattern: {}, k: {}", r, k);
                    r.matches(k)
                })
            }) {
                info!("Detected change in {}#{}!", repo.name, branch);
                let mut res = launch(LaunchOptions {
                    repo_name: repo.name.to_string(),
                    repo_url: repo.uri.to_string(),
                    branch: branch.to_string(),
                    secrets: repo.secrets.clone(),
                    environment: repo.environment.clone(),
                })?;
                res.context.repo_name = String::from(&repo.name);
                res.context.repo_url = String::from(&repo.uri);
                for notifier in &repo.notifiers {
                    notifier.send(&res)?;
                }
            }
            trace!("finished execution, persisting branch values…");
            repo.persist()?;
        }
        trace!("Waiting {:?} seconds", wait_period);
        thread::sleep(wait_period);
    }
    info!("Exiting");
    Ok(())
}

fn read_fakeci_config_file(config_file: &str) -> Result<FakeCIBinaryConfig> {
    let mut s = String::new();
    let mut f = File::open(config_file)?;
    f.read_to_string(&mut s)?;
    Ok(serde_yaml::from_str(&s)?)
}
