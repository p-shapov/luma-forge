use crate::shared::{BackgroundTask, BackgroundTaskSpawner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TauriBackgroundTaskSpawner;

impl BackgroundTaskSpawner for TauriBackgroundTaskSpawner {
    fn spawn(&self, task: BackgroundTask) {
        tauri::async_runtime::spawn(task);
    }
}
