use super::*;

#[test]
fn reads_bundled_catalogs() {
    let reader = BundledCatalogReader;

    assert!(!reader
        .workflow_catalog()
        .expect("workflow catalog")
        .workflow_presets
        .is_empty());
    assert!(!reader
        .provisioning_profiles()
        .expect("provisioning profiles")
        .is_empty());
    assert!(!reader
        .endpoint_profiles()
        .expect("endpoint profiles")
        .is_empty());
}

#[test]
fn rejects_empty_workflow_catalog() {
    let error = parse_workflow_catalog(r#"{"id":"catalog","version":"1","workflow_presets":[]}"#)
        .expect_err("empty catalog should fail");

    assert_eq!(error, WorkspaceSetupError::WorkflowCatalogUnavailable);
}

#[test]
fn rejects_malformed_profiles() {
    let error = parse_provisioning_profiles("not json").expect_err("json should fail");

    assert_eq!(error, WorkspaceSetupError::WorkflowCatalogUnavailable);
}
