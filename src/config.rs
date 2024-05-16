use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Hash, Clone)]
pub struct RunConfigurationProject {
    name: String,
    git_url: String,
    refspec: String,
    crate_root: String,
    extra_args: Option<Vec<String>>,
    prepare_script: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct RunConfiguration {
    verus_git_url: String,
    verus_refspec: String,
    verus_features: Vec<String>,
    verus_extra_args: Option<Vec<String>>,
    // #[serde(default = true)]
    // verus_verify_vstd: bool,
    #[serde(rename = "project")]
    projects: Vec<RunConfigurationProject>,
}
