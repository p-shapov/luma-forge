#[derive(Debug, Clone, Copy, Default)]
pub struct LifecycleBackgroundRunner;

impl LifecycleBackgroundRunner {
    pub fn spawn<F>(&self, task: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(task);
    }
}

#[cfg(test)]
mod tests {
    use super::LifecycleBackgroundRunner;

    #[tokio::test]
    async fn spawn_returns_while_the_task_waits_for_release() {
        let runner = LifecycleBackgroundRunner;
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let (completed_tx, mut completed_rx) = tokio::sync::oneshot::channel();

        runner.spawn(async move {
            let _ = started_tx.send(());
            let _ = release_rx.await;
            let _ = completed_tx.send(());
        });

        started_rx.await.unwrap();
        assert_eq!(
            completed_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty),
        );
        release_tx.send(()).unwrap();
        completed_rx.await.unwrap();
    }
}
