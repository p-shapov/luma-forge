use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeySetup {
    pub email: String,
    pub username: Option<String>,
    pub key_display_name: Option<String>,
}
