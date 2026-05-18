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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_blank_treats_whitespace_as_blank() {
        assert!(is_blank(""));
        assert!(is_blank(" \n\t"));
        assert!(!is_blank(" value "));
    }

    #[test]
    fn relative_paths_must_not_escape_or_be_empty() {
        for path in ["models/checkpoint.safetensors", "custom_nodes/node"] {
            assert!(is_safe_relative_path(path), "{path} should be accepted");
        }

        for path in [
            "",
            "/",
            "\\models",
            "../models",
            "models/../x",
            "models//x",
            ".",
        ] {
            assert!(!is_safe_relative_path(path), "{path} should be rejected");
        }
    }

    #[test]
    fn absolute_posix_paths_must_be_normalized_non_root_paths() {
        for path in ["/workspace", "/workspace/models"] {
            assert!(
                is_safe_absolute_posix_path(path),
                "{path} should be accepted"
            );
        }

        for path in [
            "",
            "/",
            "workspace",
            "/workspace/../x",
            "/workspace//x",
            "\\workspace",
        ] {
            assert!(
                !is_safe_absolute_posix_path(path),
                "{path} should be rejected"
            );
        }
    }
}
