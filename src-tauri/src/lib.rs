use std::fs;

use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

pub mod app;
pub mod commands;
pub mod diagnostics;
pub mod domain;
pub mod lifecycle_journal;
pub mod runpod_runtime;
pub mod runtime_catalog;
pub mod secrets;
pub mod shared;
pub mod sqlite;
pub mod workflow_catalog;
pub mod workspace_catalog;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = command_builder();

    #[cfg(debug_assertions)]
    {
        if let Err(error) = export_typescript_bindings(&builder) {
            eprintln!("failed to export TypeScript command bindings: {error}");
        }
    }

    let mut app_builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    if let Err(error) = app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            builder.mount_events(app);
            let diagnostics_guard = match app_handle.path().app_log_dir() {
                Ok(log_dir) => {
                    if let Err(error) = fs::create_dir_all(&log_dir) {
                        eprintln!(
                            "failed to create native diagnostics log directory at {}: {error}",
                            log_dir.display()
                        );
                        diagnostics::init(None)
                    } else {
                        let guard = diagnostics::init(Some(log_dir.clone()));
                        tracing::info!(
                            log_dir = %log_dir.display(),
                            "native diagnostics file logging initialized"
                        );
                        guard
                    }
                }
                Err(error) => {
                    eprintln!("native diagnostics log directory unavailable: {error}");
                    diagnostics::init(None)
                }
            };
            let app_state = match tauri::async_runtime::block_on(app::bootstrap::build_app_state(
                &app_handle,
            )) {
                Ok(state) => app::state::NativeAppState::Ready(Box::new(state)),
                Err(error) => {
                    eprintln!("native app initialization failed: {}", error.message);
                    app::state::NativeAppState::Failed(error)
                }
            };
            app.manage(diagnostics_guard);
            app.manage(app_state);
            Ok(())
        })
        .run(tauri::generate_context!())
    {
        eprintln!("error while running tauri application: {error}");
    }
}

fn command_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::native::get_native_startup_status,
            commands::catalog::get_workflow_catalog,
            commands::catalog::get_runtime_contract_catalog,
            commands::catalog::get_runpod_placement_options,
            commands::catalog::get_workspace_catalog,
            commands::secrets::setup_runpod_api_key,
            commands::secrets::get_runpod_api_key_identity,
            commands::secrets::delete_runpod_api_key,
            commands::secrets::setup_hugging_face_api_key,
            commands::secrets::get_hugging_face_api_key_identity,
            commands::secrets::delete_hugging_face_api_key,
            commands::workspaces::create_runpod_workspace,
            commands::workspaces::provision_workspace,
            commands::workspaces::cleanup_workspace,
            commands::workspaces::delete_workspace,
            commands::workspaces::get_running_lifecycle_operations,
            commands::workspaces::get_latest_lifecycle_operation
        ])
        .events(collect_events![
            commands::types::workspace::LifecycleOperationChangedEvent,
            commands::types::workspace::WorkspaceChangedEvent,
            commands::types::workspace::WorkspaceDeletedEvent
        ])
}

fn export_typescript_bindings(
    builder: &Builder<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    builder.export(
        specta_typescript::Typescript::default(),
        "../src/generated/commands.ts",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::{command_builder, export_typescript_bindings};

    #[test]
    fn export_bindings() {
        let builder = command_builder();

        export_typescript_bindings(&builder).expect("failed to export TypeScript command bindings");
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
