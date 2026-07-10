#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuggingFaceIdentity {
    pub username: String,
    pub email: Option<String>,
    pub token_display_name: Option<String>,
}
