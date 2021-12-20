/// Defines what makes for a valid configuration
use crate::Env;
use serde::{Deserialize, Serialize};

use crate::utils::docker::{rng_docker_chars, DOCKER_NAME_CHARSET};

#[cfg(test)]
mod tests {
    use crate::conf::Image;
    use crate::utils::tests::{deser_yaml, get_sample_resource_file};
    use crate::Env;

    #[test]
    fn basic_config() {
        let s = get_sample_resource_file("basic_config.yml").expect("could not find basic_config");
        let c = deser_yaml(&s).expect("could not deserialize basic config");
        assert_eq!(c.pipeline.len(), 2);
        let j0 = c.pipeline.get(0).unwrap();
        assert_eq!(j0.name, "job 0");
        assert_eq!(j0.volumes.len(), 0);
        assert_eq!(j0.env, Env::new());
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
/// Some default that may or may not be present
pub struct FakeCIDefaultConfig {
    /// An optional docker Image definition
    pub image: Option<Image>,
    #[serde(default)]
    /// default environment. Will be extended by individual jobs' envs
    pub env: Env,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
/// Represents an entire `.fakeci.yml`
pub struct FakeCIRepoConfig {
    /// A list of jobs
    pub pipeline: Vec<FakeCIJob>,
    /// Some defaults to be used if we don't want to repeat the same stuff over & over
    pub default: Option<FakeCIDefaultConfig>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
/// Represents an image we must build ourselves
pub struct FakeCIDockerBuild {
    /// Optional path to the dockerfile. Will use Dockerfile if not specified
    pub dockerfile: Option<String>,
    /// Optional context. Default: .
    pub context: Option<String>,
    /// List of build args to pass to docker build
    pub build_args: Option<Vec<String>>,
    /// Name of the image
    pub name: Option<String>,
    #[serde(default)]
    /// Should the image be privileged?
    pub privileged: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
/// Represents a docker image, with some options
pub struct FakeCIDockerImage {
    /// Name of the docker image Ex: ubuntu
    pub name: String,
    #[serde(default)]
    /// Should the image run in privileged mode?
    pub privileged: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(untagged)]
/// A docker image to use to run the [job](FakeCIJob)
pub enum Image {
    /// A simple image name. Ex: "ubuntu"
    Existing(String),
    /// A more complex image definition, with options
    ExistingFull(FakeCIDockerImage),
    /// Tells us we should build the image
    Build(FakeCIDockerBuild),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
/// Represents a Job. Serializes to:
/// ```yaml
/// name: say hello  # a name for this job.
/// image: rust  # an optional image definition. If None, must be specified via
///              # [the defaults](FakeCIRepoConfig::defaults)
/// env:
///   GREETED: "world"
/// secrets:
///   - GREETER # the actual value is defined by the inbound interface with the outside world.
///             # Specifying this only enables its use here.
/// steps:
///   - name: greets the greeted
///     exec:
///       - echo "$GREETER says: «Hello, $GREETED»"
/// ```
pub struct FakeCIJob {
    /// The job's name
    pub name: String,
    /// An optional image definition
    pub image: Option<Image>,
    /// A list of steps to execute
    pub steps: Vec<FakeCIStep>,
    #[serde(default)]
    /// Environment to pass to the steps
    pub env: Env,
    #[serde(default)]
    /// Secrets to pass to the steps. Note: actual secret definition is left to inbound interfaces
    pub secrets: Vec<String>,
    #[serde(default)]
    /// Volumes we should mount. Note: the repository is always mounted as /code
    pub volumes: Vec<String>,
}

impl FakeCIJob {
    /// Generates a random, valid, container name according to the job's name
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
/// a [job](FakeCIJob) step. Serializes to the following:
/// ```yaml
/// name: step 1 # Optional, will have an auto-generated sequential name if absent
/// exec: # a list of shell commands to execute. Each one will be executed in its own `docker start°
///   - say hello
///   - eat pie together
/// ```
pub struct FakeCIStep {
    /// An arbitrary, optional, name
    pub name: Option<String>,
    /// A list of shell commands to execute for this step
    pub exec: Vec<String>,
}

impl Image {
    /// returns if the container should be privileged according to variants
    pub fn is_privileged(&self) -> bool {
        match self {
            Image::Existing(_) => false,
            Image::ExistingFull(e) => e.privileged,
            Image::Build(b) => b.privileged,
        }
    }
    /// Returns the image's name according to variants
    pub fn get_name(&self) -> Option<String> {
        match self {
            Image::Existing(s) => Some(s.clone()),
            Image::ExistingFull(e) => Some(e.name.clone()),
            Image::Build(b) => b.name.clone(),
        }
    }
}
