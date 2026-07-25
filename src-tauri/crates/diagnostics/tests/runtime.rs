use luma_diagnostics::__private::{Field, Fields};

#[test]
fn named_fields_preserve_names_and_redaction() {
    let workspace_id = "workspace-1";
    let fields = [
        ("workspace_id", Field::shown(&workspace_id)),
        ("api_key", Field::redacted()),
    ];
    let formatted = format!("{:?}", Fields::new(&fields));
    assert!(formatted.contains("workspace_id"));
    assert!(formatted.contains("workspace-1"));
    assert!(formatted.contains("api_key"));
    assert!(formatted.contains("[REDACTED]"));
}
