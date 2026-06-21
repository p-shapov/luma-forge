use tauri::Manager;

pub mod app;
pub mod diagnostics;
pub mod domain;
pub mod lifecycle_journal;
pub mod provider;
pub mod runtime_catalog;
pub mod secrets;
pub mod sqlite;
pub mod tauri_api;
pub mod workflow_catalog;
pub mod workspace;
pub mod workspace_catalog;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri_api::builder();

    #[cfg(debug_assertions)]
    {
        let _ = tauri_api::export_typescript_bindings(&builder);
    }

    let mut app_builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    let _ = app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let app_identifier = app_handle.config().identifier.clone();
            builder.mount_events(app);
            let support_paths = tauri_api::support::prepare_support_paths(&app_handle);
            let app_state = match support_paths.and_then(|support_paths| {
                tauri::async_runtime::block_on(app::bootstrap::build_app_state(
                    &app_identifier,
                    &support_paths,
                    std::sync::Arc::new(tauri_api::events::TauriWorkspaceEventSink::new(
                        app_handle.clone(),
                    )),
                ))
            }) {
                Ok(state) => app::state::NativeAppState::Ready(Box::new(state)),
                Err(error) => app::state::NativeAppState::Failed(error),
            };
            app.manage(app_state);
            Ok(())
        })
        .run(tauri::generate_context!());
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::tauri_api;

    #[test]
    fn export_bindings() {
        let builder = tauri_api::builder();

        tauri_api::export_typescript_bindings(&builder)
            .expect("failed to export TypeScript command bindings");
    }

    #[test]
    fn production_rust_does_not_use_direct_panic_primitives() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut violations = Vec::new();

        for path in rust_source_files(&src_dir) {
            if is_test_support_file(&path) {
                continue;
            }

            let source = fs::read_to_string(&path).expect("rust source should be readable");
            let production_source = source
                .split("#[cfg(test)]")
                .next()
                .unwrap_or(source.as_str());

            for (line_index, line) in production_source.lines().enumerate() {
                if direct_panic_patterns()
                    .iter()
                    .any(|pattern| line.contains(pattern))
                {
                    let relative = path
                        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                        .expect("source path should be under manifest dir");
                    violations.push(format!(
                        "{}:{}: {}",
                        relative.display(),
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "production panic primitives found:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn tauri_events_are_mounted_before_app_state_bootstrap() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source should be readable");
        let mount_events_index = source
            .find("builder.mount_events(app)")
            .expect("Tauri Specta events should be mounted");
        let bootstrap_index = source
            .find("app::bootstrap::build_app_state")
            .expect("app state bootstrap should be present");

        assert!(
            mount_events_index < bootstrap_index,
            "Tauri Specta events must be mounted before app state bootstrap because startup stale-operation recovery emits runtime events"
        );
    }

    fn rust_source_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        collect_rust_source_files(root, &mut files);
        files
    }

    fn collect_rust_source_files(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).expect("source directory should be readable") {
            let entry = entry.expect("source directory entry should be readable");
            let path = entry.path();

            if path.is_dir() {
                collect_rust_source_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn is_test_support_file(path: &Path) -> bool {
        path.file_name()
            .is_some_and(|file_name| file_name == "tests.rs" || file_name == "test_support.rs")
    }

    fn direct_panic_patterns() -> &'static [&'static str] {
        &[
            ".unwrap()",
            ".expect(",
            "panic!(",
            "todo!(",
            "unreachable!(",
        ]
    }
}
