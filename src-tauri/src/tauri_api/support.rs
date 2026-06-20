use std::fs;

use tauri::{AppHandle, Manager};

use crate::app::{errors::AppInitializationError, support::SupportPaths};

const LOGS_DIR: &str = "logs";
const NATIVE_DB_FILE: &str = "native.sqlite";

pub fn prepare_support_paths(
    app_handle: &AppHandle,
) -> Result<SupportPaths, AppInitializationError> {
    let root_dir = app_handle.path().app_data_dir().map_err(|error| {
        AppInitializationError::AppDataDirectoryUnavailable {
            message: error.to_string(),
        }
    })?;
    fs::create_dir_all(&root_dir).map_err(|error| {
        AppInitializationError::AppDataDirectoryCreateFailed {
            path: root_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let logs_dir = root_dir.join(LOGS_DIR);
    fs::create_dir_all(&logs_dir).map_err(|error| {
        AppInitializationError::AppDataDirectoryCreateFailed {
            path: logs_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let native_db_path = root_dir.join(NATIVE_DB_FILE);

    Ok(SupportPaths::new(root_dir, logs_dir, native_db_path))
}
