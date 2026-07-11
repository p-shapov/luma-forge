#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    RunpodApiKey,
    HuggingFaceApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStatus {
    Missing,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub key_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
}
