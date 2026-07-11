mod client;
#[allow(clippy::derivable_impls)]
pub mod generated;

pub use client::HuggingFaceClient;
pub use generated::WhoamiResponse;
