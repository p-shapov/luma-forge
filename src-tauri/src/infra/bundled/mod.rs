mod catalog;
pub mod entries;
pub mod errors;
#[allow(clippy::large_enum_variant)]
pub mod generated;

pub use catalog::Catalog;
pub use errors::BundledCatalogError;
