use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerImage {
    pub docker_image_ref: String,
    pub docker_image_digest: String,
}

pub type EnvironmentVariables = BTreeMap<String, String>;
