#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainValidationError;

pub type DomainValidationResult = Result<(), DomainValidationError>;
