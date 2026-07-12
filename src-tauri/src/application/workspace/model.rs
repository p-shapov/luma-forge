use time::OffsetDateTime;

use crate::application::runtimes::{CatalogRef, RuntimeKind};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Workspace {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub workflow: CatalogRef,
    #[diagnostic(show)]
    pub created_at: OffsetDateTime,
    #[diagnostic(show)]
    pub runtime: Option<RuntimeKind>,
}
