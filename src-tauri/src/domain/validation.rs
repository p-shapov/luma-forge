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
