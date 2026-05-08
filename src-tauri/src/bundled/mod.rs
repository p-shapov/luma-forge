pub mod bundled_catalog_contracts;
mod bundled_catalog_error;
mod bundled_catalog_parser;
pub mod bundled_catalog_reader;
mod bundled_catalog_validator;

#[cfg(test)]
#[path = "bundled_catalog_tests.rs"]
mod bundled_catalog_tests;
