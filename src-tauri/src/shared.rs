use std::{
    collections::HashSet,
    future::Future,
    hash::Hash,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

pub type AppFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type BackgroundTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub trait BackgroundTaskSpawner: Send + Sync {
    fn spawn(&self, task: BackgroundTask);
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
