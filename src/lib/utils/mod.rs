use crate::conf::FakeCIJob;
use crate::{FakeCIRepoConfig, IMAGE};
use anyhow::{Error, Result};

pub mod docker;

#[cfg(test)]
pub mod tests {
    use std::env::{current_dir, set_current_dir};
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use crate::FakeCIRepoConfig;
    use anyhow::Result;
    use log::debug;


    pub fn serialize(conf: &FakeCIRepoConfig) -> Result<String> {
        Ok(serde_yaml::to_string(conf)?)
    }
    pub fn deserialize(s: &str) -> Result<FakeCIRepoConfig> {
        Ok(serde_yaml::from_str(s)?)
    }
    pub fn get_sample_resource_file(p: &str) -> Result<String> {
        let mut s = String::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let file_path = root.join("resources/tests").join(p);
        let mut f = File::open(file_path)?;
        let _count = f.read_to_string(&mut s);
        Ok(s)
    }

    pub fn with_dir<F>(path: &Path, f:F) where F: FnOnce() {
        use lazy_static::lazy_static;
        use std::sync::Mutex;
        lazy_static!{
            static ref WITH_DIR_MUTEX: Mutex<u8> = Mutex::new(0u8);
        }
        let _lock = WITH_DIR_MUTEX.lock().expect("Could not aquire lock in with_dir");
        let old_path = current_dir().expect("could not get current dir");
        debug!("path: {}", old_path.display());
        if path != old_path {
            let _ = set_current_dir(&path);
            debug!("new path: {}", path.display());
        }
        f();
        if path != old_path {
            let _ = set_current_dir(&old_path);
            debug!("new path: {}", old_path.display());
        }
    }
}


#[allow(dead_code)]
/// Trims newlines (\r & \n) from the given string
/// ```rust
/// use fakeci::utils::trim_newline;
/// let mut s = "hi!\n".to_string();
/// trim_newline(&mut s);
/// assert_eq!(s, "hi!");
/// ```
pub fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

pub fn get_job_image_or_default<'a>(job: &'a FakeCIJob, config: &'a FakeCIRepoConfig) -> Result<&'a IMAGE> {
    for j in &config.pipeline {
        if j == job {
            if j.image.is_some() {
                return Ok(j.image.as_ref().unwrap());
            }
            else if config.default.is_some() {
                if config.default.as_ref().unwrap().image.is_some() {
                    return Ok(config.default.as_ref().unwrap().image.as_ref().unwrap());
                }
            }
        }
    }
    Err(Error::msg("Could not find the given job in the config"))
}