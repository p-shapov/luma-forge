mod client;
#[allow(clippy::derivable_impls)]
mod generated;
mod types;

pub use client::HuggingFaceClient;
pub use types::{IdentityRequest, IdentityResponse};
