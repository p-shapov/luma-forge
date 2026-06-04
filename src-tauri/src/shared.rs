use std::{future::Future, pin::Pin};

pub type AppFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

pub fn is_safe_relative_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') || path.contains('\\') {
        return false;
    }

    path.split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
