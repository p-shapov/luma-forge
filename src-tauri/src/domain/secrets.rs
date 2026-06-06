use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyIdentity {
    pub email: Option<String>,
    pub username: Option<String>,
    pub key_display_name: Option<String>,
}
