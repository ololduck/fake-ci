use std::collections::HashMap;

use crate::utils::docker::{rng_docker_chars, DOCKER_NAME_CHARSET};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use crate::conf::IMAGE;
    use crate::utils::tests::{deserialize, get_sample_resource_file};

    #[test]
    fn basic_config() {
        let s = get_sample_resource_file("basic_config.yml").expect("could not find basic_config");
        let c = deserialize(&s).expect("could not deserialize basic config");
        assert_eq!(c.pipeline.len(), 2);
        let j0 = c.pipeline.get(0).unwrap();
        assert_eq!(j0.name, "job 0");
        assert_eq!(j0.volumes, None);
        assert_eq!(j0.env, None);
        assert_eq!(j0.image, Some(IMAGE::Existing("ubuntu".to_string())));
        assert_eq!(j0.steps.len(), 2);
    }

    #[test]
    fn docker_build() {
        let c = deserialize(
            &get_sample_resource_file("docker_build.yml").expect("could not find docker_build"),
        )
        .expect("could not parse docker_build");
        let j0 = c.pipeline.get(0).unwrap();
        assert!(j0.image.is_some());
        let image = j0.image.as_ref().unwrap();
        match image {
            IMAGE::Existing(s) => {
                panic!("got invalid image variant: {:?}", s);
            }
            IMAGE::Build(i) => {
                assert_eq!(i.dockerfile, Some("Dockerfile".to_string()));
                assert_eq!(i.context, Some(".".to_string()));
            }
            IMAGE::ExistingFull(s) => {
                panic!("got invalid image variant: {:?}", s);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIDefaultConfig {
    pub image: Option<IMAGE>,
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
pub enum IMAGE {
    Existing(String),
    ExistingFull(FakeCIDockerImage),
    Build(FakeCIDockerBuild),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIJob {
    pub name: String,
    pub image: Option<IMAGE>,
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

impl IMAGE {
    pub fn is_privileged(&self) -> bool {
        match self {
            IMAGE::Existing(_) => false,
            IMAGE::ExistingFull(e) => e.privileged,
            IMAGE::Build(b) => b.privileged,
        }
    }
    pub fn get_name(&self) -> Option<String> {
        match self {
            IMAGE::Existing(s) => Some(s.clone()),
            IMAGE::ExistingFull(e) => Some(e.name.clone()),
            IMAGE::Build(b) => b.name.clone(),
        }
    }
}
