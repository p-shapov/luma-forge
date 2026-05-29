use secrecy::{ExposeSecret, SecretString};

use super::{run_blocking_secret_operation, BlockingSecretStore, SecretStoreFuture};
use crate::secrets::SecretStoreError;

#[derive(Clone)]
pub struct ProvisionerWorkerBearerToken(SecretString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerBearerTokenError;

impl std::fmt::Debug for ProvisionerWorkerBearerToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProvisionerWorkerBearerToken([REDACTED])")
    }
}

impl ProvisionerWorkerBearerToken {
    pub fn new(value: String) -> Result<Self, ProvisionerWorkerBearerTokenError> {
        if value.trim().is_empty() {
            return Err(ProvisionerWorkerBearerTokenError);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

pub trait ProvisionerTokenStore: Send + Sync {
    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError>;

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError>;

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError>;
}

pub trait AsyncProvisionerTokenStore: Send + Sync {
    fn write_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> SecretStoreFuture<'a, ()>;

    fn read_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, Option<ProvisionerWorkerBearerToken>>;

    fn delete_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, ()>;
}

impl<S> AsyncProvisionerTokenStore for BlockingSecretStore<S>
where
    S: ProvisionerTokenStore + Clone + Send + Sync + 'static,
{
    fn write_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        let workspace_id = workspace_id.to_string();
        let token = token.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || {
                store.write_provisioner_worker_token(&workspace_id, &token)
            })
            .await
        })
    }

    fn read_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, Option<ProvisionerWorkerBearerToken>> {
        let store = self.store.clone();
        let workspace_id = workspace_id.to_string();
        Box::pin(async move {
            run_blocking_secret_operation(move || {
                store.read_provisioner_worker_token(&workspace_id)
            })
            .await
        })
    }

    fn delete_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        let workspace_id = workspace_id.to_string();
        Box::pin(async move {
            run_blocking_secret_operation(move || {
                store.delete_provisioner_worker_token(&workspace_id)
            })
            .await
        })
    }
}

#[cfg(test)]
impl<S> AsyncProvisionerTokenStore for S
where
    S: ProvisionerTokenStore + Send + Sync,
{
    fn write_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move {
            ProvisionerTokenStore::write_provisioner_worker_token(self, workspace_id, token)
        })
    }

    fn read_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, Option<ProvisionerWorkerBearerToken>> {
        Box::pin(
            async move { ProvisionerTokenStore::read_provisioner_worker_token(self, workspace_id) },
        )
    }

    fn delete_provisioner_worker_token<'a>(
        &'a self,
        workspace_id: &'a str,
    ) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move {
            ProvisionerTokenStore::delete_provisioner_worker_token(self, workspace_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_token(value: &str) -> ProvisionerWorkerBearerToken {
        ProvisionerWorkerBearerToken::new(value.to_string()).expect("worker token should be valid")
    }

    #[test]
    fn provisioner_worker_bearer_token_rejects_blank_values() {
        assert_eq!(
            ProvisionerWorkerBearerToken::new(" \n\t".to_string()).map(|_| ()),
            Err(ProvisionerWorkerBearerTokenError)
        );
    }

    #[test]
    fn provisioner_worker_bearer_token_exposes_secret_only_explicitly() {
        let token = worker_token("worker-secret");

        assert_eq!(token.expose_secret(), "worker-secret");
        assert_eq!(
            format!("{token:?}"),
            "ProvisionerWorkerBearerToken([REDACTED])"
        );
    }

    #[tokio::test]
    async fn blocking_provisioner_token_store_delegates_operations_and_preserves_errors() {
        #[derive(Clone)]
        struct WorkerTokenStore {
            result: Result<(), SecretStoreError>,
        }

        impl ProvisionerTokenStore for WorkerTokenStore {
            fn write_provisioner_worker_token(
                &self,
                _workspace_id: &str,
                _token: &ProvisionerWorkerBearerToken,
            ) -> Result<(), SecretStoreError> {
                self.result.clone()
            }

            fn read_provisioner_worker_token(
                &self,
                _workspace_id: &str,
            ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
                self.result.clone()?;
                Ok(Some(worker_token("worker-secret")))
            }

            fn delete_provisioner_worker_token(
                &self,
                _workspace_id: &str,
            ) -> Result<(), SecretStoreError> {
                self.result.clone()
            }
        }

        let store = BlockingSecretStore::new(WorkerTokenStore {
            result: Err(SecretStoreError::InvalidStoredProvisionerWorkerToken),
        });

        assert_eq!(
            AsyncProvisionerTokenStore::write_provisioner_worker_token(
                &store,
                "workspace-1",
                &worker_token("worker-secret"),
            )
            .await,
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken)
        );
        assert_eq!(
            AsyncProvisionerTokenStore::read_provisioner_worker_token(&store, "workspace-1")
                .await
                .map(|_| ()),
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken)
        );
        assert_eq!(
            AsyncProvisionerTokenStore::delete_provisioner_worker_token(&store, "workspace-1")
                .await,
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken)
        );
    }
}
