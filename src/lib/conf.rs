use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::utils::docker::{rng_docker_chars, DOCKER_NAME_CHARSET};

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
    pub default: Option<FakeCIDefaultConfig>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FakeCIDockerBuild {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
    pub build_args: Option<Vec<String>>,
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
