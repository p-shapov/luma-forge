use std::{future::Future, pin::Pin, sync::Arc};

use sqlx::{Row, SqlitePool};

use crate::domain::provider_setup::{
    GpuCloudProviderId, ProviderSetupError, ProviderSetupMetadata,
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait ProviderSetupRepository: Send + Sync {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<ProviderSetupMetadata>, ProviderSetupError>>;

    fn save<'a>(
        &'a self,
        metadata: ProviderSetupMetadata,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>>;

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>>;
}

#[derive(Clone)]
pub(crate) struct SharedProviderSetupRepository(Arc<dyn ProviderSetupRepository>);

impl SharedProviderSetupRepository {
    pub(crate) fn new(repository: impl ProviderSetupRepository + 'static) -> Self {
        Self(Arc::new(repository))
    }
}

impl ProviderSetupRepository for SharedProviderSetupRepository {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<ProviderSetupMetadata>, ProviderSetupError>> {
        self.0.get(provider_id)
    }

    fn save<'a>(
        &'a self,
        metadata: ProviderSetupMetadata,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        self.0.save(metadata)
    }

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        self.0.delete(provider_id)
    }
}

#[derive(Clone)]
pub(crate) struct SqliteProviderSetupRepository {
    pool: SqlitePool,
}

impl SqliteProviderSetupRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl ProviderSetupRepository for SqliteProviderSetupRepository {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<ProviderSetupMetadata>, ProviderSetupError>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT provider_id, provider_user_id, provider_api_key_fingerprint \
                 FROM provider_setup WHERE provider_id = ?",
            )
            .bind(provider_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;

            let Some(row) = row else {
                return Ok(None);
            };

            let provider_id_value: String = row
                .try_get("provider_id")
                .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;
            let provider_id = GpuCloudProviderId::parse(&provider_id_value)?;
            let provider_user_id = row
                .try_get("provider_user_id")
                .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;
            let provider_api_key_fingerprint = row
                .try_get("provider_api_key_fingerprint")
                .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;

            Ok(Some(ProviderSetupMetadata {
                provider_id,
                provider_user_id,
                provider_api_key_fingerprint,
            }))
        })
    }

    fn save<'a>(
        &'a self,
        metadata: ProviderSetupMetadata,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO provider_setup \
                 (provider_id, provider_user_id, provider_api_key_fingerprint) \
                 VALUES (?, ?, ?) \
                 ON CONFLICT(provider_id) DO UPDATE SET \
                 provider_user_id = excluded.provider_user_id, \
                 provider_api_key_fingerprint = excluded.provider_api_key_fingerprint, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            )
            .bind(metadata.provider_id.as_str())
            .bind(metadata.provider_user_id)
            .bind(metadata.provider_api_key_fingerprint)
            .execute(&self.pool)
            .await
            .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;

            Ok(())
        })
    }

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        Box::pin(async move {
            sqlx::query("DELETE FROM provider_setup WHERE provider_id = ?")
                .bind(provider_id.as_str())
                .execute(&self.pool)
                .await
                .map_err(|_| ProviderSetupError::LocalStorageUnavailable)?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database;

    #[tokio::test]
    async fn saves_and_reads_provider_setup_metadata() {
        let pool = database::connect_in_memory()
            .await
            .expect("in-memory database should initialize");
        let repository = SqliteProviderSetupRepository::new(pool);
        let metadata = ProviderSetupMetadata {
            provider_id: GpuCloudProviderId::RunPod,
            provider_user_id: "user-123".to_owned(),
            provider_api_key_fingerprint: "rpa_key_id".to_owned(),
        };

        repository
            .save(metadata)
            .await
            .expect("metadata save should succeed");
        let stored = repository
            .get(&GpuCloudProviderId::RunPod)
            .await
            .expect("metadata read should succeed")
            .expect("metadata should exist");

        assert_eq!(stored.provider_id, GpuCloudProviderId::RunPod);
        assert_eq!(stored.provider_user_id, "user-123");
        assert_eq!(stored.provider_api_key_fingerprint, "rpa_key_id");
    }

    #[tokio::test]
    async fn returns_none_when_metadata_is_missing() {
        let pool = database::connect_in_memory()
            .await
            .expect("in-memory database should initialize");
        let repository = SqliteProviderSetupRepository::new(pool);

        let stored = repository
            .get(&GpuCloudProviderId::RunPod)
            .await
            .expect("metadata read should succeed");

        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn upserts_duplicate_provider_setup_metadata() {
        let pool = database::connect_in_memory()
            .await
            .expect("in-memory database should initialize");
        let repository = SqliteProviderSetupRepository::new(pool);
        let metadata = ProviderSetupMetadata {
            provider_id: GpuCloudProviderId::RunPod,
            provider_user_id: "user-123".to_owned(),
            provider_api_key_fingerprint: "rpa_key_id".to_owned(),
        };

        repository
            .save(metadata.clone())
            .await
            .expect("first metadata save should succeed");
        repository
            .save(ProviderSetupMetadata {
                provider_id: GpuCloudProviderId::RunPod,
                provider_user_id: "user-456".to_owned(),
                provider_api_key_fingerprint: "rpa_key_id_2".to_owned(),
            })
            .await
            .expect("duplicate metadata should update");
        let stored = repository
            .get(&GpuCloudProviderId::RunPod)
            .await
            .expect("metadata read should succeed")
            .expect("metadata should exist");

        assert_eq!(stored.provider_user_id, "user-456");
        assert_eq!(stored.provider_api_key_fingerprint, "rpa_key_id_2");
    }

    #[tokio::test]
    async fn deletes_provider_setup_metadata() {
        let pool = database::connect_in_memory()
            .await
            .expect("in-memory database should initialize");
        let repository = SqliteProviderSetupRepository::new(pool);
        let metadata = ProviderSetupMetadata {
            provider_id: GpuCloudProviderId::RunPod,
            provider_user_id: "user-123".to_owned(),
            provider_api_key_fingerprint: "rpa_key_id".to_owned(),
        };

        repository
            .save(metadata)
            .await
            .expect("metadata save should succeed");
        repository
            .delete(&GpuCloudProviderId::RunPod)
            .await
            .expect("metadata delete should succeed");
        let stored = repository
            .get(&GpuCloudProviderId::RunPod)
            .await
            .expect("metadata read should succeed");

        assert!(stored.is_none());
    }
}
