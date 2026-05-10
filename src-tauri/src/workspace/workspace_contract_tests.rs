use crate::{
    bundled::bundled_catalog_reader::BundledCatalogReader,
    domain::{
        provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId,
        workspace::Workspace as DomainWorkspace,
    },
    workspace::workspace_setup_service::WorkspaceSetupCatalogReader,
};

use super::{PlacementPlan, Workspace, WorkspaceLifecycleState};

fn sample_placement_plan() -> PlacementPlan {
    let reader = BundledCatalogReader;
    PlacementPlan {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
        selected_workflow_preset: reader
            .workflow_catalog()
            .expect("workflow catalog")
            .workflow_presets
            .remove(0),
        selected_provisioning_profile: reader
            .provisioning_profiles()
            .expect("provisioning profiles")
            .remove(0),
        selected_endpoint_profile: reader
            .endpoint_profiles()
            .expect("endpoint profiles")
            .remove(0),
    }
}

#[test]
fn maps_domain_draft_workspace_to_serializable_contract() {
    let placement_plan = sample_placement_plan();
    let domain_workspace = DomainWorkspace::new_draft(
        DomainGpuCloudProviderId::Runpod,
        "018f6a40-0000-7000-8000-000000000001".to_string(),
        "Workspace".to_string(),
        placement_plan.to_domain(),
    )
    .expect("domain workspace");

    let workspace: Workspace = domain_workspace.into();

    assert_eq!(workspace.id, "018f6a40-0000-7000-8000-000000000001");
    assert_eq!(workspace.name, "Workspace");
    assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft);
    assert_eq!(workspace.placement_plan, placement_plan);
    assert!(workspace.persistent_storage_volume_snapshot.is_none());
    assert!(workspace.active_provisioning_pod_snapshot.is_none());
    assert!(workspace.serverless_endpoint_snapshot.is_none());
    assert!(workspace.last_provisioning_pod_snapshot.is_none());
    assert_eq!(workspace.environment_prepared_at, None);
}
