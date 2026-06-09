use std::future::Future;

pub fn spawn_lifecycle_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(future);
}
