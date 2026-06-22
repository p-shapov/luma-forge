use logforth::kv::{KeyView, ValueView, Visitor};
use serde::Serialize;
use serde_json::Map;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::sanitization::{sanitize_diagnostic_string, sanitize_json_value_for_key};

#[derive(Debug, Clone, Default)]
pub(super) struct SanitizingJsonLayout;

#[derive(Debug, Default)]
struct JsonValueCollector {
    kvs: Map<String, serde_json::Value>,
}

impl Visitor for JsonValueCollector {
    fn visit(&mut self, key: KeyView, value: ValueView) -> Result<(), logforth::Error> {
        let key = key.to_string();
        let value = json_value_for_log_kv(key.as_str(), value);
        self.kvs.insert(key, value);
        Ok(())
    }
}

fn json_value_for_log_kv(key: &str, value: ValueView) -> serde_json::Value {
    if key == "error" {
        if let Some(string) = value.to_str() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(string) {
                return sanitize_json_value_for_key(Some(key), value);
            }
        }
    }

    match serde_json::to_value(&value) {
        Ok(value) => sanitize_json_value_for_key(Some(key), value),
        Err(_) => {
            sanitize_json_value_for_key(Some(key), serde_json::Value::String(value.to_string()))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SanitizedRecordLine {
    timestamp: String,
    level: &'static str,
    target: String,
    file: String,
    line: u32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    ctx: Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    diags: Map<String, serde_json::Value>,
}

impl logforth::Layout for SanitizingJsonLayout {
    fn format(
        &self,
        record: &logforth::record::Record,
        diags: &[Box<dyn logforth::Diagnostic>],
    ) -> Result<Vec<u8>, logforth::Error> {
        let timestamp: OffsetDateTime = record.time().into();
        let timestamp = timestamp
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

        let mut ctx_collector = JsonValueCollector::default();
        record.key_values().visit(&mut ctx_collector)?;

        let mut diags_collector = JsonValueCollector::default();
        for diagnostic in diags {
            diagnostic.visit(&mut diags_collector)?;
        }

        let record_line = SanitizedRecordLine {
            timestamp,
            level: record.level().name(),
            target: record.target().to_string(),
            file: record.file().unwrap_or_default().to_string(),
            line: record.line().unwrap_or_default(),
            message: sanitize_diagnostic_string(&record.payload().to_string()),
            error: ctx_collector.kvs.remove("error"),
            ctx: ctx_collector.kvs,
            diags: diags_collector.kvs,
        };

        serde_json::to_vec(&record_line).map_err(|error| {
            logforth::Error::new("failed to serialize sanitized diagnostics log record")
                .with_source(error)
        })
    }
}
