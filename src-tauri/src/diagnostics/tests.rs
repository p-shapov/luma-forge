use super::*;
use std::error::Error;
use std::fmt;

use logforth::filter::FilterResult;
use logforth::kv::{KeyOwned, ValueOwned};
use logforth::record::{FilterCriteria, Level};
use logforth::Filter;
use logforth::Layout;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum TestError {
    UnitFailure,
    StructuredFailure {
        message: String,
        #[serde(skip)]
        source: Option<Box<TestError>>,
    },
}

impl fmt::Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestError::UnitFailure => formatter.write_str("unit failure"),
            TestError::StructuredFailure { message, .. } => formatter.write_str(message),
        }
    }
}

impl Error for TestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TestError::UnitFailure => None,
            TestError::StructuredFailure { source, .. } => source
                .as_ref()
                .map(|source| source.as_ref() as &(dyn Error + 'static)),
        }
    }
}

impl HasDiagnosticCode for TestError {
    fn diagnostic_code(&self) -> &'static str {
        match self {
            TestError::UnitFailure => "unit_failure",
            TestError::StructuredFailure { .. } => "structured_failure",
        }
    }

    fn diagnostic_source(&self) -> Option<&dyn HasDiagnosticCode> {
        match self {
            TestError::UnitFailure => None,
            TestError::StructuredFailure { source, .. } => source
                .as_ref()
                .map(|source| source.as_ref() as &dyn HasDiagnosticCode),
        }
    }
}

#[test]
fn error_diagnostics_shape_includes_chain() {
    let diagnostics = ErrorDiagnostics {
        code: "structured_failure".to_string(),
        message: "top-level".to_string(),
        chain: vec![ErrorDiagnosticFrame {
            code: "structured_failure".to_string(),
            message: "top-level".to_string(),
        }],
    };

    assert_eq!(diagnostics.code, "structured_failure");
    assert_eq!(diagnostics.message, "top-level");
    assert_eq!(
        diagnostics.chain,
        vec![ErrorDiagnosticFrame {
            code: "structured_failure".to_string(),
            message: "top-level".to_string(),
        }]
    );
}

#[test]
fn error_diagnostics_reports_code_message_and_chain() {
    let error = TestError::StructuredFailure {
        message: "top-level".to_string(),
        source: Some(Box::new(TestError::StructuredFailure {
            message: "middle".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "leaf".to_string(),
                source: None,
            })),
        })),
    };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(diagnostics.code, "structured_failure");
    assert_eq!(diagnostics.message, "top-level");
    assert_eq!(
        diagnostics.chain,
        vec![
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "top-level".to_string(),
            },
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "middle".to_string(),
            },
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "leaf".to_string(),
            },
        ]
    );
}

#[test]
fn error_diagnostics_reports_unit_variant_code() {
    let diagnostics = error_diagnostics(&TestError::UnitFailure);

    assert_eq!(diagnostics.code, "unit_failure");
    assert_eq!(diagnostics.message, "unit failure");
    assert_eq!(
        diagnostics.chain,
        vec![ErrorDiagnosticFrame {
            code: "unit_failure".to_string(),
            message: "unit failure".to_string(),
        }]
    );
}

#[test]
fn error_diagnostics_redacts_sensitive_strings_in_message() {
    let error = TestError::StructuredFailure {
            message: "request failed for Bearer top-secret-token at https://example.com/download?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef&foo=bar".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "Authorization: Basic super-secret".to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: "worker rejected hf_abcdefghijklmnopqrstuvwxyz123456 access_token=native-secret".to_string(),
                    source: None,
                })),
            })),
        };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(diagnostics.code, "structured_failure");
    assert_eq!(
        diagnostics.message,
        "request failed for Bearer [REDACTED] at [REDACTED_URL]"
    );
    assert_eq!(
        diagnostics.chain,
        vec![
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "request failed for Bearer [REDACTED] at [REDACTED_URL]".to_string(),
            },
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "Authorization: [REDACTED]".to_string(),
            },
            ErrorDiagnosticFrame {
                code: "structured_failure".to_string(),
                message: "worker rejected [REDACTED] access_token=[REDACTED]".to_string(),
            },
        ]
    );
    assert!(!diagnostics.message.contains("top-secret-token"));
    assert!(!diagnostics.message.contains("deadbeef"));
    assert!(!diagnostics.message.contains("https://example.com/download"));
}

#[test]
fn error_diagnostics_redacts_colon_and_quoted_sensitive_key_forms_in_message() {
    let error = TestError::StructuredFailure {
        message:
            r#"api_key: secret-one "access_token":"secret-two" authorization: Bearer secret-three"#
                .to_string(),
        source: Some(Box::new(TestError::StructuredFailure {
            message: r#""token":"secret-four""#.to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: r#"provider said access_token: secret-five"#.to_string(),
                source: None,
            })),
        })),
    };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(
        diagnostics.message,
        r#"api_key: [REDACTED] "access_token":"[REDACTED]" authorization: [REDACTED]"#
    );
}

#[test]
fn error_diagnostics_suppresses_body_like_and_oversized_strings() {
    let oversized = "x".repeat(5000);
    let error = TestError::StructuredFailure {
        message: oversized,
        source: Some(Box::new(TestError::StructuredFailure {
            message:
                r#"provider response body: {"status":"ok","items":[1,2,3],"nested":{"step":"run"}}"#
                    .to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: r#"request body: {"prompt":"draw","workflow":{"id":"wf_123"}}"#
                    .to_string(),
                source: None,
            })),
        })),
    };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(diagnostics.message, "[REDACTED_LARGE_DIAGNOSTIC]");
}

#[test]
fn error_diagnostics_keeps_safe_bracketed_messages_visible() {
    let error = TestError::StructuredFailure {
        message: "[workspace invalid] missing endpoint".to_string(),
        source: Some(Box::new(TestError::StructuredFailure {
            message: "{workspace invalid}".to_string(),
            source: None,
        })),
    };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(diagnostics.message, "[workspace invalid] missing endpoint");
}

#[test]
fn error_diagnostics_suppresses_plain_text_body_markers() {
    let error = TestError::StructuredFailure {
        message: "request body: not json but sensitive".to_string(),
        source: Some(Box::new(TestError::StructuredFailure {
            message: "payload: plain text secret".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "response body: not-json".to_string(),
                source: None,
            })),
        })),
    };

    let diagnostics = error_diagnostics(&error);

    assert_eq!(diagnostics.message, "[REDACTED_BODY]");
}

#[test]
fn sanitizing_json_layout_redacts_payload_and_key_values() {
    let layout = SanitizingJsonLayout;
    let key_values = [
            (
                KeyOwned::new("payload"),
                ValueOwned::str("provider response body: not json but sensitive"),
            ),
            (
                KeyOwned::new("download_url"),
                ValueOwned::str(
                    "https://example.com/download?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef",
                ),
            ),
        ];
    let record = logforth::record::Record::builder()
        .level(Level::Info)
        .target_static("luma_forge_lib::diagnostics")
        .file_static("src/diagnostics.rs")
        .line(Some(123))
        .payload(format_args!("Authorization: Bearer top-secret-token"))
        .key_values(&key_values[..])
        .build();

    let formatted = layout.format(&record, &[]).expect("record should format");
    let json: serde_json::Value =
        serde_json::from_slice(&formatted).expect("formatted line should be valid json");

    assert_eq!(json["message"], "Authorization: [REDACTED]");
    assert_eq!(json["ctx"]["payload"], "[REDACTED_BODY]");
    assert_eq!(json["ctx"]["download_url"], "[REDACTED_URL]");
    assert!(!formatted
        .windows("top-secret-token".len())
        .any(|window| window == b"top-secret-token"));
    assert!(!formatted
        .windows("deadbeef".len())
        .any(|window| window == b"deadbeef"));
}

#[test]
fn sanitizing_json_layout_redacts_nested_structured_key_values() {
    let layout = SanitizingJsonLayout;
    let request_value = ValueOwned::map([
        (
            KeyOwned::new("authorization"),
            ValueOwned::str("Basic secret"),
        ),
        (
            KeyOwned::new("nested"),
            ValueOwned::map([
                (KeyOwned::new("api_key"), ValueOwned::str("abc")),
                (KeyOwned::new("access_token"), ValueOwned::str("xyz")),
            ]),
        ),
        (
            KeyOwned::new("body"),
            ValueOwned::map([
                (KeyOwned::new("prompt"), ValueOwned::str("draw")),
                (
                    KeyOwned::new("workflow"),
                    ValueOwned::map([(KeyOwned::new("id"), ValueOwned::str("wf_123"))]),
                ),
            ]),
        ),
        (
            KeyOwned::new("payload"),
            ValueOwned::list([ValueOwned::map([(
                KeyOwned::new("token"),
                ValueOwned::str("array-secret"),
            )])]),
        ),
        (
            KeyOwned::new("safe"),
            ValueOwned::str("Bearer top-secret-token"),
        ),
    ]);
    let key_values = [(KeyOwned::new("details"), request_value)];
    let record = logforth::record::Record::builder()
        .level(Level::Info)
        .target_static("luma_forge_lib::diagnostics")
        .file_static("src/diagnostics.rs")
        .line(Some(123))
        .payload(format_args!("structured request"))
        .key_values(&key_values[..])
        .build();

    let formatted = layout.format(&record, &[]).expect("record should format");
    let json: serde_json::Value =
        serde_json::from_slice(&formatted).expect("formatted line should be valid json");

    assert_eq!(json["ctx"]["details"]["authorization"], "[REDACTED]");
    assert_eq!(json["ctx"]["details"]["nested"]["api_key"], "[REDACTED]");
    assert_eq!(
        json["ctx"]["details"]["nested"]["access_token"],
        "[REDACTED]"
    );
    assert_eq!(json["ctx"]["details"]["body"], "[REDACTED_BODY]");
    assert_eq!(json["ctx"]["details"]["payload"], "[REDACTED_BODY]");
    assert_eq!(json["ctx"]["details"]["safe"], "Bearer [REDACTED]");
    assert!(!formatted
        .windows("Basic secret".len())
        .any(|window| window == b"Basic secret"));
    assert!(!formatted
        .windows("array-secret".len())
        .any(|window| window == b"array-secret"));
}

#[test]
fn sanitizing_json_layout_redacts_colon_and_quoted_sensitive_key_forms() {
    let layout = SanitizingJsonLayout;
    let key_values = [(
        KeyOwned::new("details"),
        ValueOwned::str(r#"api_key: secret-one "token":"secret-two""#),
    )];
    let record = logforth::record::Record::builder()
        .level(Level::Info)
        .target_static("luma_forge_lib::diagnostics")
        .file_static("src/diagnostics.rs")
        .line(Some(123))
        .payload(format_args!(r#"authorization: Bearer secret-three"#))
        .key_values(&key_values[..])
        .build();

    let formatted = layout.format(&record, &[]).expect("record should format");
    let json: serde_json::Value =
        serde_json::from_slice(&formatted).expect("formatted line should be valid json");

    assert_eq!(json["message"], "authorization: [REDACTED]");
    assert_eq!(
        json["ctx"]["details"],
        r#"api_key: [REDACTED] "token":"[REDACTED]""#
    );
}

#[test]
fn sanitizing_json_layout_lifts_error_key_value_to_top_level() {
    let layout = SanitizingJsonLayout;
    let key_values = [(
        KeyOwned::new("error"),
        ValueOwned::map([
            (
                KeyOwned::new("code"),
                ValueOwned::str("runtime_provider_api_key_unavailable"),
            ),
            (
                KeyOwned::new("message"),
                ValueOwned::str("runpod placement options failed"),
            ),
            (
                KeyOwned::new("chain"),
                ValueOwned::list([
                    ValueOwned::map([
                        (
                            KeyOwned::new("code"),
                            ValueOwned::str("runpod_placement_options_failed"),
                        ),
                        (
                            KeyOwned::new("message"),
                            ValueOwned::str("runpod placement options failed"),
                        ),
                    ]),
                    ValueOwned::map([
                        (
                            KeyOwned::new("code"),
                            ValueOwned::str("runtime_provider_api_key_unavailable"),
                        ),
                        (
                            KeyOwned::new("message"),
                            ValueOwned::str("runtime provider api key unavailable"),
                        ),
                    ]),
                    ValueOwned::map([
                        (
                            KeyOwned::new("code"),
                            ValueOwned::str("secure_storage_unavailable"),
                        ),
                        (
                            KeyOwned::new("message"),
                            ValueOwned::str("secure storage is unavailable"),
                        ),
                    ]),
                ]),
            ),
        ]),
    )];
    let record = logforth::record::Record::builder()
        .level(Level::Error)
        .target_static("luma_forge_lib::diagnostics")
        .file_static("src/diagnostics.rs")
        .line(Some(123))
        .payload(format_args!("tauri command failed"))
        .key_values(&key_values[..])
        .build();

    let formatted = layout.format(&record, &[]).expect("record should format");
    let json: serde_json::Value =
        serde_json::from_slice(&formatted).expect("formatted line should be valid json");

    assert_eq!(
        json["error"]["code"],
        "runtime_provider_api_key_unavailable"
    );
    assert_eq!(json["error"]["message"], "runpod placement options failed");
    assert_eq!(json["error"]["chain"].as_array().unwrap().len(), 3);
    assert!(json["ctx"].get("error").is_none());
}

#[test]
fn sanitizing_json_layout_lifts_json_string_error_key_value_to_top_level() {
    let error = TestError::StructuredFailure {
        message: "top-level".to_string(),
        source: Some(Box::new(TestError::StructuredFailure {
            message: "leaf".to_string(),
            source: None,
        })),
    };
    let error_json = error_diagnostics_log_json(&error);
    assert!(error_json.starts_with(r#"{"code":"structured_failure""#));
    assert!(!error_json.contains("ErrorDiagnostics"));

    let layout = SanitizingJsonLayout;
    let key_values = [(KeyOwned::new("error"), ValueOwned::str(error_json))];
    let record = logforth::record::Record::builder()
        .level(Level::Error)
        .target_static("luma_forge_lib::diagnostics")
        .file_static("src/diagnostics.rs")
        .line(Some(123))
        .payload(format_args!("tauri command failed"))
        .key_values(&key_values[..])
        .build();

    let formatted = layout.format(&record, &[]).expect("record should format");
    let json: serde_json::Value =
        serde_json::from_slice(&formatted).expect("formatted line should be valid json");

    assert_eq!(json["error"]["code"], "structured_failure");
    assert_eq!(json["error"]["message"], "top-level");
    assert_eq!(json["error"]["chain"].as_array().unwrap().len(), 2);
    assert!(json["ctx"].get("error").is_none());
}

#[test]
fn app_log_filter_rejects_non_app_targets_and_sub_info_levels() {
    let filter = AppLogFilter;

    let app_info = FilterCriteria::builder()
        .level(Level::Info)
        .target("luma_forge_lib::provider")
        .build();
    let app_error = FilterCriteria::builder()
        .level(Level::Error)
        .target("luma_forge::tauri")
        .build();
    let app_debug = FilterCriteria::builder()
        .level(Level::Debug)
        .target("luma_forge_lib::provider")
        .build();
    let dependency_info = FilterCriteria::builder()
        .level(Level::Info)
        .target("reqwest::connect")
        .build();

    assert_eq!(filter.enabled(&app_info, &[]), FilterResult::Neutral);
    assert_eq!(filter.enabled(&app_error, &[]), FilterResult::Neutral);
    assert_eq!(filter.enabled(&app_debug, &[]), FilterResult::Reject);
    assert_eq!(filter.enabled(&dependency_info, &[]), FilterResult::Reject);
}
