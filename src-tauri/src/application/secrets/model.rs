#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    RunpodApiKey,
    HuggingFaceApiKey,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStatus {
    Missing,
    Configured,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Identity {
    #[diagnostic(show)]
    pub key_name: Option<String>,
    #[diagnostic(show)]
    pub username: Option<String>,
    #[diagnostic(show)]
    pub email: Option<String>,
}
