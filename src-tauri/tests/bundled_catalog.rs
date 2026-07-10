use std::path::Path;

use luma_forge_lib::infra::bundled::{
    entries::{execution_schemas, runtime_contracts, runtime_presets, workflows},
    Catalog,
};

#[path = "bundled_catalog/validation.rs"]
mod validation;

use validation::validate;

fn mapping_fixture() -> Catalog {
    Catalog::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bundled_catalog"))
}

#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    validate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
        .await
        .unwrap();
}

#[tokio::test]
async fn entry_mappings_read_owned_models() {
    let catalog = mapping_fixture();

    assert_eq!(workflows::Entry::all(&catalog).await.unwrap().len(), 1);
    assert_eq!(
        runtime_contracts::Entry::all(&catalog).await.unwrap().len(),
        1
    );
    assert_eq!(
        runtime_presets::Entry::all(&catalog).await.unwrap().len(),
        1
    );
    assert_eq!(
        execution_schemas::Entry::all(&catalog).await.unwrap().len(),
        1
    );

    let workflow = workflows::Entry::get(&catalog, ("test-workflow", "1.0.0"))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(workflow.id, "test-workflow");
    assert_eq!(workflow.revision, "1.0.0");
    assert!(workflows::Entry::get(&catalog, ("missing", "1.0.0"))
        .await
        .unwrap()
        .is_none());
}
