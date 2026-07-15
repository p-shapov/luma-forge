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
