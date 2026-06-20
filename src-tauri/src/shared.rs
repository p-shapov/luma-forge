use std::{
    collections::HashSet,
    error::Error,
    future::Future,
    hash::Hash,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

pub type AppFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type BackgroundTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub trait BackgroundTaskSpawner: Send + Sync {
    fn spawn(&self, task: BackgroundTask);
}

pub fn new_trace_id() -> String {
    format!("trace-{}", uuid::Uuid::new_v4())
}

pub fn leaf_error_message(error: &(dyn Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(error) = source {
        message = error.to_string();
        source = error.source();
    }

    message
}

pub fn error_source_chain(error: &(dyn Error + 'static)) -> Vec<String> {
    let mut sources = Vec::new();
    let mut source = error.source();

    while let Some(error) = source {
        sources.push(error.to_string());
        source = error.source();
    }

    sources
}

pub fn serialized_error_code(error: &impl Serialize, fallback: &'static str) -> String {
    match serde_json::to_value(error) {
        Ok(serde_json::Value::String(code)) => code,
        Ok(serde_json::Value::Object(fields)) => fields
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| fallback.to_string()),
        _ => fallback.to_string(),
    }
}

pub fn spawn_background_task<F>(spawner: &dyn BackgroundTaskSpawner, future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    spawner.spawn(Box::pin(future));
}

pub trait EventSink<T>: Send + Sync {
    fn emit(&self, event: T);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoopEventSink<T> {
    _event: PhantomData<fn(T)>,
}

impl<T> NoopEventSink<T> {
    pub const fn new() -> Self {
        Self {
            _event: PhantomData,
        }
    }
}

impl<T> Default for NoopEventSink<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventSink<T> for NoopEventSink<T> {
    fn emit(&self, _event: T) {}
}

#[derive(Debug, Clone)]
pub struct InFlightRegistry<T> {
    ids: Arc<Mutex<HashSet<T>>>,
}

impl<T> Default for InFlightRegistry<T> {
    fn default() -> Self {
        Self {
            ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl<T> InFlightRegistry<T>
where
    T: Eq + Hash + Clone,
{
    pub fn try_register(&self, id: &T) -> bool {
        self.ids().insert(id.clone())
    }

    pub fn complete(&self, id: &T) {
        self.ids().remove(id);
    }

    fn ids(&self) -> MutexGuard<'_, HashSet<T>> {
        match self.ids.lock() {
            Ok(ids) => ids,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum ApiError {
    #[error("api request was unauthorized")]
    Unauthorized,
    #[error("api request has insufficient permissions")]
    InsufficientPermissions,
    #[error("api request was rate limited")]
    RateLimited,
    #[error("api request timed out")]
    Timeout,
    #[error("api request failed: {message}")]
    RequestFailed { message: String },
}

pub fn map_api_transport_error<E>(error: reqwest::Error, wrap: impl FnOnce(ApiError) -> E) -> E {
    if error.is_timeout() {
        wrap(ApiError::Timeout)
    } else {
        wrap(ApiError::RequestFailed {
            message: error.to_string(),
        })
    }
}

pub fn map_api_status_error<E>(
    provider_name: &str,
    status: StatusCode,
    wrap: impl FnOnce(ApiError) -> E,
) -> Option<E> {
    if status.is_success() {
        return None;
    }

    let error = match status {
        StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
        StatusCode::FORBIDDEN => ApiError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => ApiError::RateLimited,
        _ => ApiError::RequestFailed {
            message: format!("{provider_name} API request failed"),
        },
    };

    Some(wrap(error))
}
