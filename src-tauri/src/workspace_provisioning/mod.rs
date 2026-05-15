mod contracts;
mod coordinator;
pub mod error;
mod failure;
mod gateways;
mod progress;
mod service;
mod snapshots;

pub use contracts::{
    CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
    CreateServerlessEndpointInput, DiscoverProvisioningPodsInput, EndpointTemplateObservation,
    NetworkVolumeObservation, ObserveProvisioningPodInput, ProvisioningPodObservation,
    ServerlessEndpointObservation, WorkspaceProvisioningResult,
};
pub use coordinator::WorkspaceProvisioningCoordinator;
pub use error::WorkspaceProvisioningError;
pub use gateways::{ProviderProvisioningGateway, ProvisionerWorkerGateway};
pub use service::{WorkspaceProvisioningConfig, WorkspaceProvisioningService};

#[cfg(test)]
pub(crate) use snapshots::runpod_template_snapshot;

#[cfg(test)]
mod tests;
