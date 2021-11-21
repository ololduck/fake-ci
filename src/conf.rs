use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct FakeCIRepoConfig {
    pub(crate) pipeline: Vec<FakeCIJob>,
    pub(crate) artefacts: Option<Vec<String>>
}

#[derive(Serialize, Deserialize)]
pub(crate) struct FakeCIJob {
    pub(crate) name: String,
    pub(crate) steps: Vec<FakeCIStep>,
    pub(crate) env: Option<HashMap<String, String>>,
    pub(crate) depends_on: Option<Vec<Self>>
}


#[derive(Serialize, Deserialize)]
pub(crate) struct FakeCIStep {
    pub(crate) name: String,
    pub(crate) execute: Vec<String>
}