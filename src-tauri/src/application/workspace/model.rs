use time::OffsetDateTime;

use crate::application::runtimes::{CatalogRef, RuntimeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub workflow: CatalogRef,
    pub created_at: OffsetDateTime,
    pub runtime: Option<RuntimeKind>,
}
