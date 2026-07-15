use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FacadeMappingError {
    #[error("timestamp cannot be represented as RFC3339")]
    InvalidTimestamp,
}

pub(super) fn timestamp(value: OffsetDateTime) -> Result<String, FacadeMappingError> {
    value
        .format(&Rfc3339)
        .map_err(|_| FacadeMappingError::InvalidTimestamp)
}
