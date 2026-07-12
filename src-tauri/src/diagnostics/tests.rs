use super::{DiagnosticDebug, DiagnosticValue, Field, Fields};
use std::sync::{Mutex, Once, OnceLock};

#[derive(Clone)]
struct Record {
    message: String,
    fields: String,
    keys: String,
}

struct Recorder;

impl log::Log for Recorder {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let mut fields = String::new();
        let mut keys = String::new();
        record
            .key_values()
            .visit(&mut StringVisitor {
                values: &mut fields,
                keys: &mut keys,
            })
            .unwrap();
        records().lock().unwrap().push(Record {
            message: record.args().to_string(),
            fields,
            keys,
        });
    }

    fn flush(&self) {}
}

struct StringVisitor<'a> {
    values: &'a mut String,
    keys: &'a mut String,
}

impl<'kvs> log::kv::VisitSource<'kvs> for StringVisitor<'_> {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        use std::fmt::Write;
        write!(self.values, "{value:?};").unwrap();
        write!(self.keys, "{key};").unwrap();
        Ok(())
    }
}

fn records() -> &'static Mutex<Vec<Record>> {
    static RECORDS: OnceLock<Mutex<Vec<Record>>> = OnceLock::new();
    RECORDS.get_or_init(Default::default)
}

fn init_test_logger() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&Recorder).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
    });
}

fn records_for(function: &str) -> Vec<Record> {
    records()
        .lock()
        .unwrap()
        .iter()
        .filter(|record| record.fields.contains(function))
        .cloned()
        .collect()
}

#[derive(Debug)]
struct TestError;

impl DiagnosticValue for TestError {}

#[super::diagnostic(show_output, show_error)]
async fn successful(
    #[diagnostic(show)] id: String,
    #[diagnostic(redact)] secret: String,
    omitted: String,
) -> Result<String, TestError> {
    let _ = (secret, omitted);
    return Ok(id);
}

#[super::diagnostic]
async fn failing() -> Result<(), TestError> {
    Err(TestError)
}

#[tokio::test]
async fn diagnostic_logs_start_then_success_with_selected_values() {
    init_test_logger();
    let result = successful("workspace-1".into(), "secret".into(), "hidden".into())
        .await
        .unwrap();
    assert_eq!(result, "workspace-1");
    let records = records_for("successful");
    assert_eq!(
        records
            .iter()
            .map(|record| record.message.as_str())
            .collect::<Vec<_>>(),
        vec!["call.start", "call.success"]
    );
    assert!(records[0].fields.contains("workspace-1"));
    assert!(records[0].fields.contains("[REDACTED]"));
    assert!(!records[0].fields.contains(": \"secret\""));
    assert!(!records[0].fields.contains("hidden"));
}

#[tokio::test]
async fn diagnostic_logs_error_type_without_omitted_error_value() {
    init_test_logger();
    assert!(failing().await.is_err());
    let records = records_for("failing");
    assert_eq!(
        records
            .iter()
            .map(|record| record.message.as_str())
            .collect::<Vec<_>>(),
        vec!["call.start", "call.error"]
    );
    assert!(records[1].keys.contains("error_type"));
    assert!(!records[1].keys.split(';').any(|key| key == "error"));
}

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
