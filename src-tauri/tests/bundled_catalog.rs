use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use luma_forge_lib::infra::bundled::{
    entries::{execution_schemas, runtime_contracts, runtime_presets, workflows},
    BundledCatalogError, Catalog,
};

fn catalog() -> Catalog {
    Catalog::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
}

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn catalog_construction_performs_no_io() {
    let _catalog = Catalog::new("missing-bundled-root");
}

#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    catalog().validate().await.unwrap();
}

#[tokio::test]
async fn entry_mappings_read_owned_models() {
    let catalog = catalog();

    assert_eq!(workflows::Entry::all(&catalog).await.unwrap().len(), 1);
    assert_eq!(
        runtime_contracts::Entry::all(&catalog).await.unwrap().len(),
        2
    );
    assert_eq!(
        runtime_presets::Entry::all(&catalog).await.unwrap().len(),
        1
    );
    assert_eq!(
        execution_schemas::Entry::all(&catalog).await.unwrap().len(),
        1
    );

    let workflow = workflows::Entry::get(&catalog, ("comfyui-hidream-o1-dev", "1.0.0"))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
    assert_eq!(workflow.revision, "1.0.0");
    assert!(workflows::Entry::get(&catalog, ("missing", "1.0.0"))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn get_reads_only_the_selected_revision_without_schemas() {
    let temp_root = copy_catalog_fixture();
    let workflows_root = temp_root.join("catalog/entries/workflows");
    copy_dir_all(
        &workflows_root.join("comfyui-hidream-o1-dev/1.0.0"),
        &workflows_root.join("selected/1.0.0"),
    );
    fs::write(
        workflows_root.join("comfyui-hidream-o1-dev/1.0.0/metadata"),
        "{",
    )
    .unwrap();
    fs::remove_dir_all(temp_root.join("catalog/schemas")).unwrap();

    let model = workflows::Entry::get(&Catalog::new(&temp_root), ("selected", "1.0.0"))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(model.id, "selected");
    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn get_rejects_a_missing_selected_document() {
    let temp_root = copy_catalog_fixture();
    fs::remove_file(
        temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata"),
    )
    .unwrap();

    assert!(matches!(
        workflows::Entry::get(
            &Catalog::new(&temp_root),
            ("comfyui-hidream-o1-dev", "1.0.0"),
        )
        .await,
        Err(BundledCatalogError::Contract { .. })
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn get_rejects_unsafe_keys_as_catalog_contract_errors() {
    assert!(matches!(
        workflows::Entry::get(&catalog(), ("../outside", "1.0.0")).await,
        Err(BundledCatalogError::Contract { .. })
    ));

    for key in [
        ("../comfyui-hidream-o1-dev", "1.0.0"),
        ("comfyui-hidream-o1-dev", "../1.0.0"),
    ] {
        assert!(matches!(
            workflows::Entry::get(&catalog(), key).await,
            Err(BundledCatalogError::Contract { path, .. }) if path == "catalog/entries"
        ));
    }
}

#[tokio::test]
async fn audit_rejects_a_dangling_reference() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root
        .join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements");
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace("\"id\": \"provisioner\"", "\"id\": \"missing\"");
    fs::write(&path, value).unwrap();

    assert!(matches!(
        Catalog::new(&temp_root).validate().await,
        Err(BundledCatalogError::UnresolvedReference {
            contract,
            id,
            revision,
            ..
        }) if contract == "catalog/contracts/runtime_contract_revision"
            && id == "missing"
            && revision == "1.0.0"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn audit_rejects_a_missing_contract_schema_without_revisions() {
    let temp_root = copy_catalog_fixture();
    fs::remove_dir_all(temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev")).unwrap();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    let value = fs::read_to_string(&path).unwrap().replace(
        "luma-forge://schema/workflow_metadata",
        "luma-forge://schema/missing",
    );
    fs::write(&path, value).unwrap();

    assert!(matches!(
        Catalog::new(&temp_root).validate().await,
        Err(BundledCatalogError::Schema { path, .. })
            if path == "catalog/contracts/workflow_revision"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn reads_reject_a_retired_contract_field() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    let value = fs::read_to_string(&path).unwrap().replacen(
        '{',
        "{\n  \"entity\": \"workflow_revision\",",
        1,
    );
    fs::write(&path, value).unwrap();

    assert!(matches!(
        workflows::Entry::all(&Catalog::new(&temp_root)).await,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/contracts/workflow_revision"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn audit_and_reads_reject_a_symlinked_contract() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../new_bundled/catalog/contracts/workflow_revision"),
        &path,
    )
    .unwrap();

    let read = workflows::Entry::all(&Catalog::new(&temp_root)).await;
    let audit = Catalog::new(&temp_root).validate().await;

    assert!(matches!(
        read,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/contracts/workflow_revision"
    ));
    assert!(matches!(
        audit,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/contracts/workflow_revision"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn audit_and_reads_reject_a_symlinked_revision() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0");
    fs::remove_dir_all(&path).unwrap();
    std::os::unix::fs::symlink(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"),
        &path,
    )
    .unwrap();

    let read = workflows::Entry::get(
        &Catalog::new(&temp_root),
        ("comfyui-hidream-o1-dev", "1.0.0"),
    )
    .await;
    let audit = Catalog::new(&temp_root).validate().await;

    assert!(matches!(
        read,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"
    ));
    assert!(matches!(
        audit,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn reads_reject_an_entries_path_outside_catalog_entries() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace("catalog/entries/workflows", "catalog/entries/../outside");
    fs::write(&path, value).unwrap();

    assert!(matches!(
        workflows::Entry::all(&Catalog::new(&temp_root)).await,
        Err(BundledCatalogError::Contract { .. })
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

fn copy_catalog_fixture() -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled");
    let target = std::env::temp_dir().join(format!(
        "luma-forge-bundled-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
    ));
    copy_dir_all(&source, &target);
    target
}

fn copy_dir_all(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
