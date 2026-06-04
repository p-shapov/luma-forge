use std::{future::Future, pin::Pin};

pub type AppFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
