mod context;
mod coordinator;
pub(crate) mod failure;
pub(crate) mod helpers;
mod service;
mod steps;

pub use coordinator::WorkspaceProvisioningCoordinator;
pub use helpers::{WorkspaceProvisioningError, WorkspaceProvisioningResult};
pub use service::{WorkspaceProvisioningConfig, WorkspaceProvisioningService};

#[cfg(test)]
pub(crate) use crate::domain::workspace::provisioning_state::runpod_template_snapshot;

#[cfg(test)]
mod tests;
