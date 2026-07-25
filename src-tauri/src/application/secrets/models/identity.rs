#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Identity {
    #[diagnostic(show)]
    pub key_name: Option<String>,
    #[diagnostic(show)]
    pub username: Option<String>,
    #[diagnostic(show)]
    pub email: Option<String>,
}
