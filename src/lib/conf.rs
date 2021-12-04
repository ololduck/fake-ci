use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};

use anyhow::Result;
use glob;
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};

use crate::utils::cache_dir;
use crate::utils::docker::{rng_docker_chars, DOCKER_NAME_CHARSET};
use crate::utils::git::fetch;

#[cfg(test)]
mod tests {
    use crate::conf::Image;
    use crate::utils::tests::{deser_yaml, get_sample_resource_file};

    #[test]
    fn basic_config() {
        let s = get_sample_resource_file("basic_config.yml").expect("could not find basic_config");
        let c = deser_yaml(&s).expect("could not deserialize basic config");
        assert_eq!(c.pipeline.len(), 2);
        let j0 = c.pipeline.get(0).unwrap();
        assert_eq!(j0.name, "job 0");
        assert_eq!(j0.volumes, None);
        assert_eq!(j0.env, None);
        assert_eq!(j0.image, Some(Image::Existing("ubuntu".to_string())));
        assert_eq!(j0.steps.len(), 2);
    }

    #[test]
    fn docker_build() {
        let c = deser_yaml(
            &get_sample_resource_file("docker_build.yml").expect("could not find docker_build"),
        )
        .expect("could not parse docker_build");
        let j0 = c.pipeline.get(0).unwrap();
        assert!(j0.image.is_some());
        let image = j0.image.as_ref().unwrap();
        match image {
            Image::Existing(s) => {
                panic!("got invalid image variant: {:?}", s);
            }
            Image::Build(i) => {
                assert_eq!(i.dockerfile, Some("Dockerfile".to_string()));
                assert_eq!(i.context, Some(".".to_string()));
            }
            Image::ExistingFull(s) => {
                panic!("got invalid image variant: {:?}", s);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIDefaultConfig {
    pub image: Option<Image>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIRepoConfig {
    pub pipeline: Vec<FakeCIJob>,
    pub artefacts: Option<Vec<String>>,
    pub default: Option<FakeCIDefaultConfig>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIDockerBuild {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
    pub build_args: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub privileged: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIDockerImage {
    pub name: String,
    #[serde(default)]
    pub privileged: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(untagged)]
pub enum Image {
    Existing(String),
    ExistingFull(FakeCIDockerImage),
    Build(FakeCIDockerBuild),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIJob {
    pub name: String,
    pub image: Option<Image>,
    pub steps: Vec<FakeCIStep>,
    pub env: Option<HashMap<String, String>>,
    pub volumes: Option<Vec<String>>,
}

impl FakeCIJob {
    pub fn generate_container_name(&self) -> String {
        let valid_bytes = self
            .name
            .to_lowercase()
            .as_bytes()
            .iter()
            .map(|b| match b {
                b' ' => b'-',
                _ => *b,
            })
            .filter(|b| DOCKER_NAME_CHARSET.contains(b))
            .collect::<Vec<u8>>();
        let name = String::from_utf8_lossy(&valid_bytes);
        format!("fake-ci-{}-{}", name, rng_docker_chars(4))
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIStep {
    pub name: Option<String>,
    pub exec: Vec<String>,
}

impl Image {
    pub fn is_privileged(&self) -> bool {
        match self {
            Image::Existing(_) => false,
            Image::ExistingFull(e) => e.privileged,
            Image::Build(b) => b.privileged,
        }
    }
    pub fn get_name(&self) -> Option<String> {
        match self {
            Image::Existing(s) => Some(s.clone()),
            Image::ExistingFull(e) => Some(e.name.clone()),
            Image::Build(b) => b.name.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(untagged)]
pub enum BranchesSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FakeCIBinaryRepositoryConfig {
    pub name: String,
    pub uri: String,
    pub branches: BranchesSpec,
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
        let added: HashMap<String, String> = HashMap::from_iter(
            r.iter()
                .filter(|(k, _)| !self.refs.contains_key(*k))
                .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        diff.extend(added.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        for (k, v) in self.refs.iter() {
            if r.contains_key(k) && r.get(k).unwrap() != self.refs.get(k).unwrap() {
                diff.insert(k.to_string(), v.to_string());
            }
        }
        self.refs.extend(added);
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
        // find cache dir
        let cache = cache_dir();
        let mut f = File::create(cache.join(format!("{}.yml", self.name)))?;
        // write to cache dir
        let _ = f.write_all(serde_yaml::to_string(&self.refs)?.as_ref());
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Config for the binary
/// ```
/// use fakeci::conf::FakeCIBinaryConfig;
/// let s: &str = "[[repository]]\nname = \"test\"\nuri = \"http://fake.uri/\"\nbranches = [\"*\"]";
/// let c: FakeCIBinaryConfig = toml::from_str(s).expect("invalid toml");
/// assert_eq!(c.watch_interval, 300);
/// assert_eq!(c.repositories.len(), 1);
/// ```
pub struct FakeCIBinaryConfig {
    #[serde(default = "watch_interval_default")]
    pub watch_interval: u32,
    #[serde(alias = "repository")]
    pub repositories: Vec<FakeCIBinaryRepositoryConfig>,
}

fn watch_interval_default() -> u32 {
    300
}
