#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppInitializationError {
    #[error("app data directory is unavailable: {message}")]
    AppDataDirectoryUnavailable { message: String },
    #[error("app data directory could not be created at {path}: {message}")]
    AppDataDirectoryCreateFailed { path: String, message: String },
    #[error("native diagnostics could not be initialized: {message}")]
    DiagnosticsInitializationFailed { message: String },
    #[error("workspace storage could not be initialized at {path}: {message}")]
    WorkspaceStorageInitializationFailed { path: String, message: String },
    #[error("provider services could not be initialized: {message}")]
    ProviderServicesInitializationFailed { message: String },
    #[error("workspace lifecycle state could not be restored: {message}")]
    LifecycleStateRestoreFailed { message: String },
}
