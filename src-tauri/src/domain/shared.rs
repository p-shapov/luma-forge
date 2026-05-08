use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DockerImage {
    pub docker_image_ref: String,
    pub docker_image_digest: String,
}

pub type EnvironmentVariables = BTreeMap<String, String>;
