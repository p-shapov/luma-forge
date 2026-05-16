use crate::{
    domain::workspace::{ProviderProvisioningSnapshot, Workspace},
    secrets::SecretStore,
    workspace_provisioning::{ProviderProvisioningGateway, WorkspaceProvisioningError},
};

pub async fn cleanup_known_resources<S, P>(
    secrets: &S,
    providers: &P,
    workspace: &Workspace,
) -> Result<(), WorkspaceProvisioningError>
where
    S: SecretStore,
    P: ProviderProvisioningGateway,
{
    let mut first_error = None;

    if let Some(endpoint) = &workspace.serverless_endpoint_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                providers
                    .delete_serverless_endpoint(
                        workspace.gpu_cloud_provider_id,
                        &endpoint.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(template_id) = runpod_template_id(workspace) {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                providers
                    .delete_endpoint_template(workspace.gpu_cloud_provider_id, &template_id)
                    .await,
            ),
        );
    }

    if let Some(active_pod) = &workspace.active_provisioning_pod_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                providers
                    .delete_provisioning_pod(
                        workspace.gpu_cloud_provider_id,
                        &active_pod.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(volume) = &workspace.persistent_storage_volume_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                providers
                    .delete_network_volume(
                        workspace.gpu_cloud_provider_id,
                        &volume.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    remember_first_error(
        &mut first_error,
        secrets
            .delete_provisioner_worker_token(&workspace.id)
            .map_err(WorkspaceProvisioningError::from),
    );

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn runpod_template_id(workspace: &Workspace) -> Option<String> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(snapshot),
        }) => Some(snapshot.template_id.clone()),
        _ => None,
    }
}

fn tolerate_missing(
    result: Result<(), WorkspaceProvisioningError>,
) -> Result<(), WorkspaceProvisioningError> {
    match result {
        Ok(()) | Err(WorkspaceProvisioningError::ProviderResourceNotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

fn remember_first_error(
    first_error: &mut Option<WorkspaceProvisioningError>,
    result: Result<(), WorkspaceProvisioningError>,
) {
    if first_error.is_none() {
        if let Err(error) = result {
            *first_error = Some(error);
        }
    }
}
