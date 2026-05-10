use super::profiles::EnvironmentVariables;

pub(crate) fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

pub(crate) fn is_safe_relative_path(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('/') || value.starts_with('\\') {
        return false;
    }

    value
        .split(['/', '\\'])
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

pub(crate) fn is_safe_absolute_posix_path(value: &str) -> bool {
    let value = value.trim();
    if value == "/" || !value.starts_with('/') || value.contains('\\') {
        return false;
    }

    value[1..]
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

pub(crate) fn is_valid_http_path(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('?')
        && !value.contains('#')
        && !value.chars().any(char::is_whitespace)
}

pub(crate) fn is_valid_docker_image_ref(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.contains("://")
        || value.chars().any(char::is_whitespace)
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_' | '/' | ':' | '@')
        })
    {
        return false;
    }

    let Some(first_name_part) = value.split([':', '@']).next() else {
        return false;
    };

    first_name_part.split('/').all(|part| {
        part.chars()
            .any(|character| character.is_ascii_alphanumeric())
    })
}

pub(crate) fn is_valid_optional_enum(value: Option<&str>, allowed: &[&str]) -> bool {
    value
        .map(|value| allowed.contains(&value.trim()))
        .unwrap_or(true)
}

pub(crate) fn is_valid_optional_non_blank(value: Option<&str>) -> bool {
    value.map(|value| !is_blank(value)).unwrap_or(true)
}

pub(crate) fn is_valid_environment(value: Option<&EnvironmentVariables>) -> bool {
    value
        .map(|environment| {
            environment.iter().all(|(key, value)| {
                !is_blank(key) && !key.contains('=') && !key.contains('\0') && !value.contains('\0')
            })
        })
        .unwrap_or(true)
}
