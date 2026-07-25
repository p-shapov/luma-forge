use std::sync::Arc;

use crate::application::{
    events::{ApplicationEvent, ApplicationEventSink},
    runtimes::{
        ports::{RuntimePersistenceError, RuntimeTransitionRepository},
        RuntimeOperation,
    },
    workspace::Workspace,
};

#[derive(Clone)]
pub struct RuntimeTransitionContext {
    transitions: Arc<dyn RuntimeTransitionRepository>,
    events: Arc<dyn ApplicationEventSink>,
    coordinator: Arc<tokio::sync::Mutex<()>>,
}

impl RuntimeTransitionContext {
    pub fn new(
        transitions: Arc<dyn RuntimeTransitionRepository>,
        events: Arc<dyn ApplicationEventSink>,
    ) -> Self {
        Self {
            transitions,
            events,
            coordinator: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn save(
        &self,
        #[diagnostic(show)] workspace: &Workspace,
        #[diagnostic(show)] operation: &RuntimeOperation,
    ) -> Result<(), RuntimePersistenceError> {
        let _guard = self.coordinator.lock().await;
        self.transitions
            .save_transition(workspace, operation)
            .await?;
        self.events
            .emit(ApplicationEvent::WorkspaceChanged(workspace.clone()));
        self.events
            .emit(ApplicationEvent::RuntimeOperationChanged(operation.clone()));
        Ok(())
    }
}
