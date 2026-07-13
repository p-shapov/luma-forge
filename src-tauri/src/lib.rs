pub mod adapters;
pub mod application;
pub mod diagnostics;
pub mod facade;
pub mod infra;
pub mod providers;

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_specta::Event;

    #[test]
    fn facade_event_names_are_stable() {
        assert_eq!(facade::WorkspaceChangedEvent::NAME, "workspace_changed");
        assert_eq!(facade::WorkspaceDeletedEvent::NAME, "workspace_deleted");
        assert_eq!(facade::RuntimeOperationEvent::NAME, "runtime_operation");
    }

    #[test]
    fn export_bindings() {
        facade::export_typescript_bindings(&facade::builder())
            .expect("failed to export TypeScript bindings");
    }
}
