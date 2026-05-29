use sqlx::{sqlite::SqliteRow, Row, SqliteTransaction};

use crate::{
    domain::{
        placement::PlacementPlan,
        provisioner::ResolvedProvisionerImageSnapshot,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::WorkflowPreset,
        workspace::validator as workspace_validator,
        workspace::{
            PersistentStorageVolumeSnapshot, ProvisioningPodSnapshot,
            ServerlessEndpointProviderMetadata, ServerlessEndpointSnapshot, Workspace,
            WorkspaceProvisioningFailure,
        },
    },
    workspace_setup::error::WorkspaceSetupError,
};

use super::values::{
    parse_gpu_cloud_provider_id, parse_lifecycle_state, parse_provider_resource_status,
    parse_provisioning_failure_code, parse_provisioning_failure_source, parse_provisioning_phase,
    parse_provisioning_recovery_action,
};

pub(super) async fn decode_workspace(
    transaction: &mut SqliteTransaction<'_>,
    row: SqliteRow,
) -> Result<Workspace, WorkspaceSetupError> {
    let id: String = row
        .try_get("id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let name: String = row
        .try_get("name")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let provider_id = parse_gpu_cloud_provider_id(
        row.try_get::<String, _>("gpu_cloud_provider_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .as_str(),
    )?;
    let lifecycle_state = parse_lifecycle_state(
        row.try_get::<String, _>("lifecycle_state")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .as_str(),
    )?;
    let environment_prepared_at: Option<String> = row
        .try_get("environment_prepared_at")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    let placement_plan = read_placement(transaction, &id).await?;
    let resolved_runtime_image = read_runtime_image(transaction, &id).await?;
    let resolved_provisioner_image = read_provisioner_image(transaction, &id).await?;
    let persistent_storage_volume_snapshot =
        read_persistent_storage_volume_snapshot(transaction, &id).await?;
    let active_provisioning_pod_snapshot =
        read_provisioning_pod_snapshot(transaction, &id, "active_provisioning_pod").await?;
    let serverless_endpoint_snapshot = read_serverless_endpoint_snapshot(transaction, &id).await?;
    let last_provisioning_pod_snapshot =
        read_provisioning_pod_snapshot(transaction, &id, "last_provisioning_pod").await?;
    let last_provisioning_failure = read_provisioning_failure(transaction, &id).await?;

    let workspace = Workspace {
        gpu_cloud_provider_id: provider_id,
        id,
        name,
        lifecycle_state,
        placement_plan,
        resolved_runtime_image,
        resolved_provisioner_image,
        persistent_storage_volume_snapshot,
        active_provisioning_pod_snapshot,
        serverless_endpoint_snapshot,
        last_provisioning_pod_snapshot,
        environment_prepared_at,
        last_provisioning_failure,
    };

    workspace_validator::validate_workspace(&workspace)
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;

    Ok(workspace)
}

async fn read_provisioner_image(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<ResolvedProvisionerImageSnapshot, WorkspaceSetupError> {
    let row = sqlx::query(
        r#"
        SELECT contract_id, contract_version, provisioner_worker_image_ref
        FROM workspace_provisioner_images
        WHERE workspace_id = ?
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    Ok(ResolvedProvisionerImageSnapshot {
        contract_id: row
            .try_get("contract_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        contract_version: row
            .try_get("contract_version")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        provisioner_worker_image_ref: row
            .try_get("provisioner_worker_image_ref")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
    })
}

async fn read_placement(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<PlacementPlan, WorkspaceSetupError> {
    let row = sqlx::query(
        r#"
        SELECT
            selected_datacenter_id,
            selected_gpu_id,
            persistent_storage_volume_size_bytes,
            endpoint_keep_alive_seconds,
            selected_workflow_preset_id,
            selected_workflow_preset_json
        FROM workspace_runpod_placements
        WHERE workspace_id = ?
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    let selected_workflow_preset: WorkflowPreset = serde_json::from_str(
        row.try_get::<String, _>("selected_workflow_preset_json")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .as_str(),
    )
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
    let selected_workflow_preset_id: String = row
        .try_get("selected_workflow_preset_id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    if selected_workflow_preset_id != selected_workflow_preset.id {
        return Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
    }
    let storage_size: i64 = row
        .try_get("persistent_storage_volume_size_bytes")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let keep_alive: i64 = row
        .try_get("endpoint_keep_alive_seconds")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    Ok(PlacementPlan::Runpod {
        selected_datacenter_id: row
            .try_get("selected_datacenter_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        selected_gpu_id: row
            .try_get("selected_gpu_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        persistent_storage_volume_size_bytes: u64::try_from(storage_size)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        endpoint_keep_alive_seconds: u32::try_from(keep_alive)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        selected_workflow_preset,
    })
}

async fn read_runtime_image(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<ResolvedRuntimeImageSnapshot, WorkspaceSetupError> {
    let row = sqlx::query(
        r#"
        SELECT contract_id, contract_version, endpoint_image_ref
        FROM workspace_runtime_images
        WHERE workspace_id = ?
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    Ok(ResolvedRuntimeImageSnapshot {
        contract_id: row
            .try_get("contract_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        contract_version: row
            .try_get("contract_version")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        endpoint_image_ref: row
            .try_get("endpoint_image_ref")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
    })
}

async fn read_resource_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
    role: &str,
) -> Result<Option<SqliteRow>, WorkspaceSetupError> {
    sqlx::query(
        r#"
        SELECT
            gpu_cloud_provider_id,
            provider_resource_id,
            provider_resource_status,
            provisioner_status_url,
            endpoint_invoke_url,
            provider_metadata_json
        FROM workspace_resource_snapshots
        WHERE workspace_id = ? AND snapshot_role = ?
        "#,
    )
    .bind(workspace_id)
    .bind(role)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)
}

async fn read_persistent_storage_volume_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<Option<PersistentStorageVolumeSnapshot>, WorkspaceSetupError> {
    let Some(row) =
        read_resource_snapshot(transaction, workspace_id, "persistent_storage_volume").await?
    else {
        return Ok(None);
    };

    Ok(Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: parse_gpu_cloud_provider_id(
            row.try_get::<String, _>("gpu_cloud_provider_id")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        provider_resource_id: row
            .try_get("provider_resource_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        provider_resource_status: parse_provider_resource_status(
            row.try_get::<String, _>("provider_resource_status")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
    }))
}

async fn read_provisioning_pod_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
    role: &str,
) -> Result<Option<ProvisioningPodSnapshot>, WorkspaceSetupError> {
    let Some(row) = read_resource_snapshot(transaction, workspace_id, role).await? else {
        return Ok(None);
    };

    Ok(Some(ProvisioningPodSnapshot {
        gpu_cloud_provider_id: parse_gpu_cloud_provider_id(
            row.try_get::<String, _>("gpu_cloud_provider_id")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        provider_resource_id: row
            .try_get("provider_resource_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        provider_resource_status: parse_provider_resource_status(
            row.try_get::<String, _>("provider_resource_status")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        provisioner_status_url: row
            .try_get::<Option<String>, _>("provisioner_status_url")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .ok_or(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
    }))
}

async fn read_serverless_endpoint_snapshot(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<Option<ServerlessEndpointSnapshot>, WorkspaceSetupError> {
    let Some(row) =
        read_resource_snapshot(transaction, workspace_id, "serverless_endpoint").await?
    else {
        return Ok(None);
    };

    Ok(Some(ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: parse_gpu_cloud_provider_id(
            row.try_get::<String, _>("gpu_cloud_provider_id")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        provider_resource_id: row
            .try_get("provider_resource_id")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        provider_resource_status: parse_provider_resource_status(
            row.try_get::<String, _>("provider_resource_status")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        endpoint_invoke_url: row
            .try_get::<Option<String>, _>("endpoint_invoke_url")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .ok_or(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?,
        provider_metadata: row
            .try_get::<Option<String>, _>("provider_metadata_json")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
            .map(|json| serde_json::from_str::<ServerlessEndpointProviderMetadata>(&json))
            .transpose()
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?,
    }))
}

async fn read_provisioning_failure(
    transaction: &mut SqliteTransaction<'_>,
    workspace_id: &str,
) -> Result<Option<WorkspaceProvisioningFailure>, WorkspaceSetupError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT code, phase, source, recovery_action
        FROM workspace_provisioning_failures
        WHERE workspace_id = ?
        "#,
    )
    .bind(workspace_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?
    else {
        return Ok(None);
    };

    Ok(Some(WorkspaceProvisioningFailure {
        code: parse_provisioning_failure_code(
            row.try_get::<String, _>("code")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        phase: parse_provisioning_phase(
            row.try_get::<String, _>("phase")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        source: parse_provisioning_failure_source(
            row.try_get::<String, _>("source")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
        recovery_action: parse_provisioning_recovery_action(
            row.try_get::<String, _>("recovery_action")
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?
                .as_str(),
        )?,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::{
        test_fixtures::{catalog_path, draft_workspace, provisioning_ready_and_failed_workspaces},
        SqliteWorkspaceCatalog,
    };
    use crate::{
        workspace_catalog::repository::WorkspaceCatalogRepository,
        workspace_setup::error::WorkspaceSetupError,
    };

    #[tokio::test]
    async fn find_and_list_report_corrupt_preset_snapshot() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("corrupt-preset"))
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");
        catalog
            .insert_workspace(&workspace)
            .await
            .expect("insert workspace");
        sqlx::query(
            "UPDATE workspace_runpod_placements SET selected_workflow_preset_json = ? WHERE workspace_id = ?",
        )
        .bind("{not valid json")
        .bind("workspace-a")
        .execute(&catalog.pool)
        .await
        .expect("corrupt preset snapshot");

        assert_eq!(
            catalog.find_workspace_by_id("workspace-a").await,
            Err(WorkspaceSetupError::WorkspaceCatalogCorrupt)
        );
        assert_eq!(
            catalog.list_workspaces().await,
            Err(WorkspaceSetupError::WorkspaceCatalogCorrupt)
        );
    }

    #[tokio::test]
    async fn find_and_list_report_schema_mismatch_between_normalized_rows() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("schema-mismatch"))
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");
        catalog
            .insert_workspace(&workspace)
            .await
            .expect("insert workspace");
        sqlx::query(
            "UPDATE workspace_runpod_placements SET selected_workflow_preset_id = ? WHERE workspace_id = ?",
        )
        .bind("other-preset")
        .bind("workspace-a")
        .execute(&catalog.pool)
        .await
        .expect("corrupt placement row");

        assert_eq!(
            catalog.find_workspace_by_id("workspace-a").await,
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
        assert_eq!(
            catalog.list_workspaces().await,
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[tokio::test]
    async fn round_trips_provisioning_ready_and_failed_workspace_metadata() {
        let path = catalog_path("metadata-round-trip");
        let catalog = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect("connect catalog");
        let workspaces = provisioning_ready_and_failed_workspaces();

        for workspace in &workspaces {
            assert_eq!(
                catalog
                    .insert_workspace(workspace)
                    .await
                    .expect("insert workspace"),
                *workspace
            );
        }

        let reconnected = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect("reconnect catalog");
        for workspace in workspaces {
            assert_eq!(
                reconnected
                    .find_workspace_by_id(&workspace.id)
                    .await
                    .expect("find workspace"),
                Some(workspace)
            );
        }
    }
}
