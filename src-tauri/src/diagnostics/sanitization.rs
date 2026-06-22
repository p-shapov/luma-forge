const REDACTED: &str = "[REDACTED]";
const REDACTED_BODY: &str = "[REDACTED_BODY]";
const REDACTED_LARGE_DIAGNOSTIC: &str = "[REDACTED_LARGE_DIAGNOSTIC]";
const REDACTED_URL: &str = "[REDACTED_URL]";
const MAX_DIAGNOSTIC_STRING_LEN: usize = 2048;

pub(super) fn sanitize_diagnostic_string(value: &str) -> String {
    if value.len() > MAX_DIAGNOSTIC_STRING_LEN {
        return REDACTED_LARGE_DIAGNOSTIC.to_string();
    }

    if looks_like_raw_body(value) {
        return REDACTED_BODY.to_string();
    }

    let mut sanitized = redact_signed_url_tokens(value);
    sanitized = redact_header_value(&sanitized, "authorization:");
    sanitized = redact_bearer_token(&sanitized);

    for key in plain_text_sensitive_keys() {
        sanitized = redact_key_value(&sanitized, key);
    }

    redact_hugging_face_token(&sanitized)
}

fn looks_like_raw_body(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    for marker in [
        "request body:",
        "response body:",
        "provider response body:",
        "raw provider response body:",
        "command payload:",
        "payload:",
    ] {
        if lower.contains(marker) {
            return true;
        }
    }

    false
}

pub(super) fn sanitize_json_value_for_key(
    key: Option<&str>,
    value: serde_json::Value,
) -> serde_json::Value {
    if key.is_some_and(is_body_like_json_key) {
        return suppress_json_value(value);
    }

    if key.is_some_and(is_sensitive_json_key) {
        return serde_json::Value::String(REDACTED.to_string());
    }

    match value {
        serde_json::Value::String(string) => {
            serde_json::Value::String(sanitize_diagnostic_string(&string))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(|value| sanitize_json_value_for_key(None, value))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(child_key, value)| {
                    let sanitized = sanitize_json_value_for_key(Some(child_key.as_str()), value);
                    (child_key, sanitized)
                })
                .collect(),
        ),
        other => other,
    }
}

fn suppress_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(string) => {
            if string.len() > MAX_DIAGNOSTIC_STRING_LEN {
                serde_json::Value::String(REDACTED_LARGE_DIAGNOSTIC.to_string())
            } else {
                serde_json::Value::String(REDACTED_BODY.to_string())
            }
        }
        _ => serde_json::Value::String(REDACTED_BODY.to_string()),
    }
}

fn is_sensitive_json_key(key: &str) -> bool {
    let normalized = normalize_json_key(key);

    normalized.contains("authorization")
        || normalized.contains("authheader")
        || matches_sensitive_json_key_suffix(&normalized)
}

fn is_body_like_json_key(key: &str) -> bool {
    let normalized = normalize_json_key(key);

    matches!(
        normalized.as_str(),
        "body"
            | "payload"
            | "requestbody"
            | "responsebody"
            | "providerresponse"
            | "rawproviderresponse"
            | "commandpayload"
            | "request"
            | "response"
    )
}

fn normalize_json_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn plain_text_sensitive_keys() -> &'static [&'static str] {
    &[
        "access_token",
        "api_key",
        "token",
        "worker_token",
        "provider_api_key",
        "hugging_face_key",
        "hf_token",
        "x-amz-signature",
        "x-amz-credential",
        "x-goog-signature",
    ]
}

fn matches_sensitive_json_key_suffix(normalized: &str) -> bool {
    [
        "apikey",
        "token",
        "accesstoken",
        "workertoken",
        "providerapikey",
        "huggingfacekey",
        "hftoken",
    ]
    .into_iter()
    .any(|suffix| normalized == suffix || normalized.ends_with(suffix))
}

fn redact_signed_url_tokens(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());

    for token in value.split_inclusive(char::is_whitespace) {
        let token_end = token.trim_end_matches(char::is_whitespace).len();
        let (content, suffix) = token.split_at(token_end);
        sanitized.push_str(&redact_signed_url_token(content));
        sanitized.push_str(suffix);
    }

    if sanitized.is_empty() && !value.is_empty() {
        redact_signed_url_token(value)
    } else {
        sanitized
    }
}

fn redact_signed_url_token(token: &str) -> String {
    let trimmed_end = token.trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
    let trailing = &token[trimmed_end.len()..];

    if looks_like_signed_url(trimmed_end) {
        format!("{REDACTED_URL}{trailing}")
    } else {
        token.to_string()
    }
}

fn looks_like_signed_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.contains("http://") || lower.contains("https://"))
        && [
            "x-amz-",
            "x-goog-",
            "signature=",
            "x-amz-signature=",
            "x-goog-signature=",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn redact_bearer_token(value: &str) -> String {
    redact_prefixed_secret(value, "bearer ", true)
}

fn redact_hugging_face_token(value: &str) -> String {
    redact_prefixed_secret(value, "hf_", false)
}

fn redact_prefixed_secret(value: &str, prefix: &str, keep_prefix: bool) -> String {
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], prefix) {
        let start = search_from + relative_start;
        let secret_start = start + prefix.len();
        let secret_end = find_secret_end(&sanitized, secret_start);

        if secret_start == secret_end {
            search_from = secret_start;
            continue;
        }

        let replacement = if keep_prefix {
            format!("{}{}", &sanitized[start..secret_start], REDACTED)
        } else {
            REDACTED.to_string()
        };

        sanitized.replace_range(start..secret_end, &replacement);
        search_from = start + replacement.len();
    }

    sanitized
}

fn redact_header_value(value: &str, header_name: &str) -> String {
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], header_name) {
        let start = search_from + relative_start;
        let header_end = start + header_name.len();
        let value_start = skip_spaces(&sanitized, header_end);
        let value_end = sanitized[value_start..]
            .find(['\n', '\r'])
            .map(|offset| value_start + offset)
            .unwrap_or_else(|| sanitized.len());

        sanitized.replace_range(value_start..value_end, REDACTED);
        search_from = value_start + REDACTED.len();
    }

    sanitized
}

fn redact_key_value(value: &str, key: &str) -> String {
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], key) {
        let start = search_from + relative_start;
        let key_end = start + key.len();

        if has_identifier_prefix(&sanitized, start) || has_identifier_suffix(&sanitized, key_end) {
            search_from = key_end;
            continue;
        }

        let separator_index = skip_key_suffix_delimiters(&sanitized, key_end);
        let Some(separator) = sanitized[separator_index..].chars().next() else {
            break;
        };
        if separator != ':' && separator != '=' {
            search_from = key_end;
            continue;
        }

        let value_start = skip_spaces(&sanitized, separator_index + separator.len_utf8());
        let (secret_start, secret_end) = if let Some(quote) = sanitized[value_start..]
            .chars()
            .next()
            .filter(|character| matches!(character, '"' | '\''))
        {
            let quoted_start = value_start + quote.len_utf8();
            let quoted_end = sanitized[quoted_start..]
                .find(quote)
                .map(|offset| quoted_start + offset)
                .unwrap_or_else(|| find_key_value_end(&sanitized, quoted_start));
            (quoted_start, quoted_end)
        } else {
            (value_start, find_key_value_end(&sanitized, value_start))
        };

        if secret_start == secret_end {
            search_from = value_start;
            continue;
        }

        sanitized.replace_range(secret_start..secret_end, REDACTED);
        search_from = secret_start + REDACTED.len();
    }

    sanitized
}

fn has_identifier_prefix(value: &str, index: usize) -> bool {
    value[..index]
        .chars()
        .next_back()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn has_identifier_suffix(value: &str, index: usize) -> bool {
    value[index..]
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn skip_spaces(value: &str, index: usize) -> usize {
    let mut current = index;
    while let Some(character) = value[current..].chars().next() {
        if !character.is_ascii_whitespace() || matches!(character, '\n' | '\r') {
            break;
        }
        current += character.len_utf8();
    }
    current
}

fn skip_key_suffix_delimiters(value: &str, index: usize) -> usize {
    let mut current = index;
    while let Some(character) = value[current..].chars().next() {
        if character.is_ascii_whitespace() || matches!(character, '"' | '\'') {
            current += character.len_utf8();
            continue;
        }
        break;
    }
    current
}

fn find_secret_end(value: &str, index: usize) -> usize {
    value[index..]
        .find([
            ' ', '\t', '\n', '\r', ',', ';', '"', '\'', ')', ']', '}', '<', '>',
        ])
        .map(|offset| index + offset)
        .unwrap_or_else(|| value.len())
}

fn find_key_value_end(value: &str, index: usize) -> usize {
    value[index..]
        .find(['&', ' ', '\t', '\n', '\r', ',', ';', ')', ']', '}'])
        .map(|offset| index + offset)
        .unwrap_or_else(|| value.len())
}

fn find_case_insensitive(value: &str, pattern: &str) -> Option<usize> {
    value
        .to_ascii_lowercase()
        .find(&pattern.to_ascii_lowercase())
}
