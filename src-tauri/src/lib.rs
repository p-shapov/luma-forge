use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::{collect_commands, Builder};

pub mod domain;
pub mod remote_workspace;
pub mod shared;
pub mod workflow_catalog;
pub mod workspace_catalog;

const REFACTOR_MESSAGE: &str = "Native backend refactor is in progress.";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NativeCommandError {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RefactorCommandRequest {}

type CommandResult = Result<(), NativeCommandError>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = command_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings(&builder);

    let mut app_builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn command_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        get_gpu_cloud_provider_setup,
        setup_gpu_cloud_provider,
        delete_gpu_cloud_provider_setup,
        get_hugging_face_api_key_setup,
        setup_hugging_face_api_key,
        delete_hugging_face_api_key_setup,
        get_workflow_catalog,
        get_provider_placement_options,
        get_workspace_catalog,
        create_workspace,
        initiate_workspace_provisioning,
        sync_workspace_provisioning,
        cancel_workspace_provisioning
    ])
}

fn export_typescript_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/generated/commands.ts",
        )
        .expect("failed to export TypeScript command bindings");
}

fn refactor_error() -> NativeCommandError {
    NativeCommandError {
        message: REFACTOR_MESSAGE.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
fn get_gpu_cloud_provider_setup(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn setup_gpu_cloud_provider(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn delete_gpu_cloud_provider_setup(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn get_hugging_face_api_key_setup(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn setup_hugging_face_api_key(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn delete_hugging_face_api_key_setup(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn get_workflow_catalog() -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn get_provider_placement_options(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn get_workspace_catalog() -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn create_workspace(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn initiate_workspace_provisioning(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn sync_workspace_provisioning(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[tauri::command]
#[specta::specta]
fn cancel_workspace_provisioning(_request: Option<RefactorCommandRequest>) -> CommandResult {
    Err(refactor_error())
}

#[cfg(test)]
mod tests {
    use super::{command_builder, export_typescript_bindings};

    #[test]
    fn export_bindings() {
        let builder = command_builder();

        export_typescript_bindings(&builder);
    }
}
