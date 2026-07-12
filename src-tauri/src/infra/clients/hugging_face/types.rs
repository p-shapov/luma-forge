use secrecy::SecretString;

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct IdentityRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct IdentityResponse {
    #[diagnostic(show)]
    pub key_name: Option<String>,
    #[diagnostic(show)]
    pub username: String,
    #[diagnostic(show)]
    pub email: Option<String>,
}
