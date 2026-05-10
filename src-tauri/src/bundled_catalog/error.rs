#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BundledCatalogError {
    ParseFailed,
    ValidationFailed,
}
