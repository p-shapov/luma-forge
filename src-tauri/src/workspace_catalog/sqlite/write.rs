use sqlx::SqliteTransaction;

use crate::{
    domain::{
        placement::PlacementPlan,
        provider_setup::GpuCloudProviderId,
        workspace::{ProviderProvisioningSnapshot, ProviderResourceStatus, Workspace},
    },
    workspace_setup::error::WorkspaceSetupError,
};

use super::values::{
    gpu_cloud_provider_id_value, provider_resource_status_value, provisioning_failure_code_value,
    provisioning_failure_source_value, provisioning_phase_value,
    provisioning_recovery_action_value,
};

pub(super) async fn persist_workspace_details(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    persist_placement(transaction, workspace).await?;
    persist_runtime_image(transaction, workspace).await?;
    persist_resource_snapshots(transaction, workspace).await?;
    persist_provider_provisioning_snapshot(transaction, workspace).await?;
    persist_provisioning_failure(transaction, workspace).await?;
    Ok(())
}

pub(super) async fn delete_workspace_details(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<(), WorkspaceSetupError> {
    for table in [
        "workspace_runpod_placements",
        "workspace_runtime_images",
        "workspace_resource_snapshots",
        "workspace_runpod_endpoint_templates",
        "workspace_provisioning_failures",
    ] {
        let statement = format!("DELETE FROM {table} WHERE workspace_id = ?");
        sqlx::query(&statement)
            .bind(workspace_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
    }
    Ok(())
}

async fn persist_placement(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    match &workspace.placement_plan {
        PlacementPlan::Runpod {
            selected_datacenter_id,
            selected_gpu_id,
            persistent_storage_volume_size_bytes,
            endpoint_keep_alive_seconds,
            selected_workflow_preset,
        } => {
            let selected_workflow_preset_json = serde_json::to_string(selected_workflow_preset)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            let storage_size = i64::try_from(*persistent_storage_volume_size_bytes)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            sqlx::query(
                r#"
                INSERT INTO workspace_runpod_placements (
                    workspace_id,
                    selected_datacenter_id,
                    selected_gpu_id,
                    persistent_storage_volume_size_bytes,
                    endpoint_keep_alive_seconds,
                    selected_workflow_preset_id,
                    selected_workflow_preset_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&workspace.id)
            .bind(selected_datacenter_id)
            .bind(selected_gpu_id)
            .bind(storage_size)
            .bind(i64::from(*endpoint_keep_alive_seconds))
            .bind(&selected_workflow_preset.id)
            .bind(selected_workflow_preset_json)
            .execute(&mut **transaction)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
        }
    }
    Ok(())
}

async fn persist_runtime_image(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    sqlx::query(
        r#"
        INSERT INTO workspace_runtime_images (
            workspace_id,
            contract_id,
            contract_version,
            provisioner_image_ref,
            endpoint_image_ref
        ) VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&workspace.id)
    .bind(&workspace.resolved_runtime_image.contract_id)
    .bind(&workspace.resolved_runtime_image.contract_version)
    .bind(&workspace.resolved_runtime_image.provisioner_image_ref)
    .bind(&workspace.resolved_runtime_image.endpoint_image_ref)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
    Ok(())
}

async fn persist_resource_snapshots(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    if let Some(snapshot) = &workspace.persistent_storage_volume_snapshot {
        insert_resource_snapshot(
            transaction,
            &workspace.id,
            "persistent_storage_volume",
            &snapshot.gpu_cloud_provider_id,
            &snapshot.provider_resource_id,
            &snapshot.provider_resource_status,
            Some(snapshot.mount_path.as_str()),
            None,
            None,
        )
        .await?;
    }
    if let Some(snapshot) = &workspace.active_provisioning_pod_snapshot {
        insert_resource_snapshot(
            transaction,
            &workspace.id,
            "active_provisioning_pod",
            &snapshot.gpu_cloud_provider_id,
            &snapshot.provider_resource_id,
            &snapshot.provider_resource_status,
            None,
            Some(snapshot.provisioner_status_url.as_str()),
            None,
        )
        .await?;
    }
    if let Some(snapshot) = &workspace.last_provisioning_pod_snapshot {
        insert_resource_snapshot(
            transaction,
            &workspace.id,
            "last_provisioning_pod",
            &snapshot.gpu_cloud_provider_id,
            &snapshot.provider_resource_id,
            &snapshot.provider_resource_status,
            None,
            Some(snapshot.provisioner_status_url.as_str()),
            None,
        )
        .await?;
    }
    if let Some(snapshot) = &workspace.serverless_endpoint_snapshot {
        insert_resource_snapshot(
            transaction,
            &workspace.id,
            "serverless_endpoint",
            &snapshot.gpu_cloud_provider_id,
            &snapshot.provider_resource_id,
            &snapshot.provider_resource_status,
            None,
            None,
            Some(snapshot.endpoint_invoke_url.as_str()),
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_resource_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
    snapshot_role: &str,
    gpu_cloud_provider_id: &GpuCloudProviderId,
    provider_resource_id: &str,
    provider_resource_status: &ProviderResourceStatus,
    mount_path: Option<&str>,
    provisioner_status_url: Option<&str>,
    endpoint_invoke_url: Option<&str>,
) -> Result<(), WorkspaceSetupError> {
    sqlx::query(
        r#"
        INSERT INTO workspace_resource_snapshots (
            workspace_id,
            snapshot_role,
            gpu_cloud_provider_id,
            provider_resource_id,
            provider_resource_status,
            mount_path,
            provisioner_status_url,
            endpoint_invoke_url
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(workspace_id)
    .bind(snapshot_role)
    .bind(gpu_cloud_provider_id_value(gpu_cloud_provider_id))
    .bind(provider_resource_id)
    .bind(provider_resource_status_value(provider_resource_status))
    .bind(mount_path)
    .bind(provisioner_status_url)
    .bind(endpoint_invoke_url)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
    Ok(())
}

async fn persist_provider_provisioning_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    let Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(snapshot),
    }) = &workspace.provider_provisioning_snapshot
    else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO workspace_runpod_endpoint_templates (
            workspace_id,
            template_id,
            provider_resource_status,
            endpoint_worker_image_ref,
            mount_path
        ) VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&workspace.id)
    .bind(&snapshot.template_id)
    .bind(provider_resource_status_value(
        &snapshot.provider_resource_status,
    ))
    .bind(&snapshot.endpoint_worker_image_ref)
    .bind(&snapshot.mount_path)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
    Ok(())
}

async fn persist_provisioning_failure(
    transaction: &mut SqliteTransaction<'_>,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    let Some(failure) = &workspace.last_provisioning_failure else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO workspace_provisioning_failures (
            workspace_id,
            code,
            phase,
            source,
            retryable,
            recovery_action,
            diagnostic
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&workspace.id)
    .bind(provisioning_failure_code_value(&failure.code))
    .bind(provisioning_phase_value(&failure.phase))
    .bind(provisioning_failure_source_value(&failure.source))
    .bind(if failure.retryable { 1_i64 } else { 0_i64 })
    .bind(provisioning_recovery_action_value(&failure.recovery_action))
    .bind(&failure.diagnostic)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::ProviderResourceStatus, workspace_catalog::sqlite::test_fixtures,
    };
    use sqlx::Row;

    async fn insert_workspace_row(transaction: &mut SqliteTransaction<'_>, workspace: &Workspace) {
        sqlx::query(
            r#"
            INSERT INTO workspaces (
                id,
                name,
                gpu_cloud_provider_id,
                lifecycle_state,
                created_at,
                updated_at,
                environment_prepared_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&workspace.id)
        .bind(&workspace.name)
        .bind(gpu_cloud_provider_id_value(
            &workspace.gpu_cloud_provider_id,
        ))
        .bind(super::super::values::lifecycle_state_value(
            &workspace.lifecycle_state,
        ))
        .bind("2026-05-18T00:00:00Z")
        .bind("2026-05-18T00:00:00Z")
        .bind(&workspace.environment_prepared_at)
        .execute(&mut **transaction)
        .await
        .expect("insert workspace row");
    }

    #[tokio::test]
    async fn persist_workspace_details_writes_normalized_detail_rows() {
        let pool = test_fixtures::bootstrapped_pool().await;
        let workspace = test_fixtures::ready_workspace();
        let mut transaction = pool.begin().await.expect("begin transaction");
        insert_workspace_row(&mut transaction, &workspace).await;

        persist_workspace_details(&mut transaction, &workspace)
            .await
            .expect("persist workspace details");
        transaction.commit().await.expect("commit transaction");

        let placement = sqlx::query(
            r#"
            SELECT selected_datacenter_id, selected_gpu_id, selected_workflow_preset_id
            FROM workspace_runpod_placements
            WHERE workspace_id = ?
            "#,
        )
        .bind(&workspace.id)
        .fetch_one(&pool)
        .await
        .expect("read placement row");
        assert_eq!(
            placement
                .try_get::<String, _>("selected_datacenter_id")
                .expect("datacenter"),
            "EU-RO-1"
        );
        assert_eq!(
            placement
                .try_get::<String, _>("selected_gpu_id")
                .expect("gpu"),
            "NVIDIA A40"
        );
        assert_eq!(
            placement
                .try_get::<String, _>("selected_workflow_preset_id")
                .expect("preset"),
            "preset-a"
        );

        let runtime = sqlx::query(
            r#"
            SELECT contract_id, contract_version
            FROM workspace_runtime_images
            WHERE workspace_id = ?
            "#,
        )
        .bind(&workspace.id)
        .fetch_one(&pool)
        .await
        .expect("read runtime image row");
        assert_eq!(
            runtime
                .try_get::<String, _>("contract_id")
                .expect("contract id"),
            "comfyui-python312-cu121"
        );
        assert_eq!(
            runtime
                .try_get::<String, _>("contract_version")
                .expect("contract version"),
            "1.0.0"
        );

        let resource_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM workspace_resource_snapshots
            WHERE workspace_id = ?
            "#,
        )
        .bind(&workspace.id)
        .fetch_one(&pool)
        .await
        .expect("read resource snapshot count")
        .try_get("count")
        .expect("resource snapshot count");
        assert_eq!(resource_count, 3);

        let template_status: String = sqlx::query(
            r#"
            SELECT provider_resource_status
            FROM workspace_runpod_endpoint_templates
            WHERE workspace_id = ?
            "#,
        )
        .bind(&workspace.id)
        .fetch_one(&pool)
        .await
        .expect("read endpoint template row")
        .try_get("provider_resource_status")
        .expect("template status");
        assert_eq!(template_status, "ready");
    }

    #[tokio::test]
    async fn persist_workspace_details_writes_failure_metadata() {
        let pool = test_fixtures::bootstrapped_pool().await;
        let workspace = test_fixtures::provisioning_ready_and_failed_workspaces()
            .into_iter()
            .find(|workspace| workspace.last_provisioning_failure.is_some())
            .expect("failed workspace fixture");
        let mut transaction = pool.begin().await.expect("begin transaction");
        insert_workspace_row(&mut transaction, &workspace).await;

        persist_workspace_details(&mut transaction, &workspace)
            .await
            .expect("persist workspace details");
        transaction.commit().await.expect("commit transaction");

        let active_pod_status: String = sqlx::query(
            r#"
            SELECT provider_resource_status
            FROM workspace_resource_snapshots
            WHERE workspace_id = ? AND snapshot_role = ?
            "#,
        )
        .bind(&workspace.id)
        .bind("active_provisioning_pod")
        .fetch_one(&pool)
        .await
        .expect("read active pod row")
        .try_get("provider_resource_status")
        .expect("active pod status");
        assert_eq!(
            active_pod_status,
            provider_resource_status_value(&ProviderResourceStatus::Running)
        );

        let failure = sqlx::query(
            r#"
            SELECT code, phase, source, retryable, recovery_action, diagnostic
            FROM workspace_provisioning_failures
            WHERE workspace_id = ?
            "#,
        )
        .bind(&workspace.id)
        .fetch_one(&pool)
        .await
        .expect("read failure row");
        assert_eq!(
            failure.try_get::<String, _>("code").expect("failure code"),
            "readiness_validation_failed"
        );
        assert_eq!(
            failure
                .try_get::<String, _>("phase")
                .expect("failure phase"),
            "validating_readiness"
        );
        assert_eq!(
            failure
                .try_get::<String, _>("source")
                .expect("failure source"),
            "native"
        );
        assert_eq!(
            failure
                .try_get::<i64, _>("retryable")
                .expect("failure retryable"),
            1
        );
        assert_eq!(
            failure
                .try_get::<String, _>("recovery_action")
                .expect("recovery action"),
            "retry"
        );
        assert_eq!(
            failure
                .try_get::<Option<String>, _>("diagnostic")
                .expect("failure diagnostic"),
            Some("readiness check failed".to_string())
        );
    }

    #[tokio::test]
    async fn delete_workspace_details_removes_all_normalized_detail_rows() {
        let pool = test_fixtures::bootstrapped_pool().await;
        let workspace = test_fixtures::ready_workspace();
        let mut transaction = pool.begin().await.expect("begin transaction");
        insert_workspace_row(&mut transaction, &workspace).await;
        persist_workspace_details(&mut transaction, &workspace)
            .await
            .expect("persist workspace details");

        delete_workspace_details(&mut transaction, &workspace.id)
            .await
            .expect("delete workspace details");
        transaction.commit().await.expect("commit transaction");

        for table in [
            "workspace_runpod_placements",
            "workspace_runtime_images",
            "workspace_resource_snapshots",
            "workspace_runpod_endpoint_templates",
            "workspace_provisioning_failures",
        ] {
            let statement = format!("SELECT COUNT(*) AS count FROM {table} WHERE workspace_id = ?");
            let count: i64 = sqlx::query(&statement)
                .bind(&workspace.id)
                .fetch_one(&pool)
                .await
                .expect("read detail count")
                .try_get("count")
                .expect("detail count");
            assert_eq!(count, 0, "{table} should not have rows");
        }
    }
}
