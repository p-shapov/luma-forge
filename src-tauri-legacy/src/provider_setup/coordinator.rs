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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};

    #[tokio::test]
    async fn locks_serialize_mutations_for_same_provider() {
        let coordinator = Arc::new(ProviderSetupCoordinator::default());
        let first_guard = coordinator.lock(&GpuCloudProviderId::Runpod).await;
        let second_coordinator = Arc::clone(&coordinator);
        let mut second_lock = tokio::spawn(async move {
            let _second_guard = second_coordinator.lock(&GpuCloudProviderId::Runpod).await;
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut second_lock)
                .await
                .is_err()
        );

        drop(first_guard);

        tokio::time::timeout(Duration::from_secs(1), second_lock)
            .await
            .expect("second lock should complete after first guard drops")
            .expect("second lock task should not panic");
    }
}
