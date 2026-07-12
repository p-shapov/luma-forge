use super::{DiagnosticDebug, DiagnosticValue, Field, Fields};

#[derive(DiagnosticDebug)]
#[allow(dead_code)]
struct Request {
    #[diagnostic(show)]
    workspace_id: String,
    #[diagnostic(redact)]
    api_key: String,
    body: serde_json::Value,
}

fn assert_diagnostic<T: DiagnosticValue>(_: &T) {}

#[test]
fn diagnostic_debug_shows_redacts_and_omits_fields() {
    let request = Request {
        workspace_id: "workspace-1".into(),
        api_key: "secret".into(),
        body: serde_json::json!({"large": true}),
    };
    assert_diagnostic(&request);
    let formatted = format!("{request:?}");
    assert!(formatted.contains("workspace-1"));
    assert!(formatted.contains("api_key: [REDACTED]"));
    assert!(!formatted.contains("secret"));
    assert!(!formatted.contains("body"));
}

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
