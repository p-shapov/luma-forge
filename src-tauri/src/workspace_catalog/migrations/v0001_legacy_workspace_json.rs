use sqlx::{Row, SqliteTransaction};

use crate::{
    domain::{workspace::validator as workspace_validator, workspace::Workspace},
    workspace_catalog::{
        migrations::{WorkspaceCatalogMigrationSource, CURRENT_PERSISTENCE_VERSION},
        sqlite::{decode_workspace_row, validate_workspace_row},
    },
    workspace_setup::error::WorkspaceSetupError,
};

pub(super) async fn migrate(
    transaction: &mut SqliteTransaction<'_>,
    migration_source: &WorkspaceCatalogMigrationSource,
) -> Result<(), WorkspaceSetupError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            name,
            gpu_cloud_provider_id,
            lifecycle_state,
            workflow_preset_id,
            workspace_json
        FROM workspaces
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

    for row in rows {
        if decode_workspace_row(&row).is_ok() {
            continue;
        }

        let id = workspace_id_for_diagnostics(&row);
        let workspace_json: String = row.try_get("workspace_json").map_err(|_| {
            log_migration_failure(id.as_deref(), "missing_workspace_json");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;
        let value = serde_json::from_str(&workspace_json).map_err(|_| {
            log_migration_failure(id.as_deref(), "malformed_workspace_json");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;

        let migrated = migrate_legacy_workspace_value(value, migration_source).map_err(|_| {
            log_migration_failure(id.as_deref(), "legacy_workspace_migration_failed");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;
        let workspace: Workspace = serde_json::from_value(migrated).map_err(|_| {
            log_migration_failure(id.as_deref(), "migrated_workspace_decode_failed");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;
        workspace_validator::validate_workspace(&workspace).map_err(|_| {
            log_migration_failure(id.as_deref(), "migrated_workspace_validation_failed");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;
        validate_workspace_row(&row, &workspace).map_err(|_| {
            log_migration_failure(id.as_deref(), "migrated_workspace_row_mismatch");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;

        let migrated_json = serde_json::to_string(&workspace).map_err(|_| {
            log_migration_failure(id.as_deref(), "migrated_workspace_encode_failed");
            WorkspaceSetupError::WorkspaceCatalogUnavailable
        })?;

        sqlx::query("UPDATE workspaces SET workspace_json = ? WHERE id = ?")
            .bind(migrated_json)
            .bind(&workspace.id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    }

    Ok(())
}

fn migrate_legacy_workspace_value(
    mut value: serde_json::Value,
    migration_source: &WorkspaceCatalogMigrationSource,
) -> Result<serde_json::Value, WorkspaceSetupError> {
    let provider_id = value
        .get("gpu_cloud_provider_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)?
        .to_string();
    let placement_plan = value
        .get_mut("placement_plan")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    placement_plan.insert(
        "gpu_cloud_provider_id".to_string(),
        serde_json::Value::String(provider_id),
    );

    let workflow_preset_id = selected_object_id(placement_plan, "selected_workflow_preset")?;
    let provisioning_profile_id =
        selected_object_id(placement_plan, "selected_provisioning_profile")?;
    let endpoint_profile_id = selected_object_id(placement_plan, "selected_endpoint_profile")?;

    let workflow_preset = migration_source
        .workflow_catalog
        .workflow_presets
        .iter()
        .find(|preset| preset.id == workflow_preset_id)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let provisioning_profile = migration_source
        .provisioning_profiles
        .iter()
        .find(|profile| profile.id() == provisioning_profile_id)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let endpoint_profile = migration_source
        .endpoint_profiles
        .iter()
        .find(|profile| profile.id() == endpoint_profile_id)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

    placement_plan.insert(
        "selected_workflow_preset".to_string(),
        serde_json::to_value(workflow_preset)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?,
    );
    placement_plan.insert(
        "selected_provisioning_profile".to_string(),
        serde_json::to_value(provisioning_profile)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?,
    );
    placement_plan.insert(
        "selected_endpoint_profile".to_string(),
        serde_json::to_value(endpoint_profile)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?,
    );

    Ok(value)
}

fn selected_object_id(
    placement_plan: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<String, WorkspaceSetupError> {
    placement_plan
        .get(field)
        .and_then(|value| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)
}

fn workspace_id_for_diagnostics(row: &sqlx::sqlite::SqliteRow) -> Option<String> {
    row.try_get("id").ok()
}

fn log_migration_failure(workspace_id: Option<&str>, reason: &str) {
    tracing::warn!(
        workspace_id,
        version = CURRENT_PERSISTENCE_VERSION,
        reason,
        "workspace catalog migration failed"
    );
}
