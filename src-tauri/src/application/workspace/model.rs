use time::OffsetDateTime;

use crate::application::catalog::CatalogRef;
use crate::application::runtimes::RuntimeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStatus {
    NotProvisioned,
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub workflow: CatalogRef,
    pub created_at: OffsetDateTime,
    pub attached_runtime: Option<RuntimeKind>,
}
