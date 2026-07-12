#[allow(clippy::derivable_impls)]
mod generated;
mod provider;
mod types;

pub use provider::HuggingFaceProvider;
pub use types::{IdentityRequest, IdentityResponse};
