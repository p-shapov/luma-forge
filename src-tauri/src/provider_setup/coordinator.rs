use tokio::sync::{Mutex, MutexGuard};

use crate::domain::provider_setup::GpuCloudProviderId;

#[derive(Debug, Default)]
pub struct ProviderSetupCoordinator {
    runpod: Mutex<()>,
}

impl ProviderSetupCoordinator {
    pub async fn lock(&self, provider_id: &GpuCloudProviderId) -> ProviderSetupGuard<'_> {
        match provider_id {
            GpuCloudProviderId::Runpod => ProviderSetupGuard {
                _guard: self.runpod.lock().await,
            },
        }
    }
}

pub struct ProviderSetupGuard<'a> {
    _guard: MutexGuard<'a, ()>,
}
