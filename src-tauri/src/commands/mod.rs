//! Tauri command boundary exposed to the React client.

#[macro_use]
mod provider_setup;

use provider_setup::{
    delete_gpu_cloud_provider_setup, get_gpu_cloud_provider_setup, setup_gpu_cloud_provider,
    sync_gpu_cloud_provider_setup,
};
use tauri_specta::{collect_commands, Builder};

#[cfg(any(debug_assertions, test))]
mod bindings;

#[cfg(any(debug_assertions, test))]
pub(crate) use bindings::export_typescript_bindings;

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        get_gpu_cloud_provider_setup,
        setup_gpu_cloud_provider,
        sync_gpu_cloud_provider_setup,
        delete_gpu_cloud_provider_setup
    ])
}
