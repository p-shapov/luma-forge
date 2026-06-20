use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

use crate::commands::errors::{NativeCommandError, NativeInitializationCommandError};

const LOGS_DIR: &str = "logs";
const NATIVE_DB_FILE: &str = "native.sqlite";

#[derive(Debug, Clone)]
pub struct SupportPaths {
    root_dir: PathBuf,
    logs_dir: PathBuf,
    native_db_path: PathBuf,
}

impl SupportPaths {
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn native_db_path(&self) -> &Path {
        &self.native_db_path
    }
}

pub fn prepare_support_paths(app_handle: &AppHandle) -> Result<SupportPaths, NativeCommandError> {
    let root_dir = app_handle.path().app_data_dir().map_err(|error| {
        NativeCommandError::native_initialization(
            NativeInitializationCommandError::AppDataDirectoryUnavailable {
                message: error.to_string(),
            },
        )
    })?;
    fs::create_dir_all(&root_dir).map_err(|error| {
        NativeCommandError::native_initialization(
            NativeInitializationCommandError::AppDataDirectoryCreateFailed {
                path: root_dir.display().to_string(),
                message: error.to_string(),
            },
        )
    })?;
    let logs_dir = root_dir.join(LOGS_DIR);
    fs::create_dir_all(&logs_dir).map_err(|error| {
        NativeCommandError::native_initialization(
            NativeInitializationCommandError::AppDataDirectoryCreateFailed {
                path: logs_dir.display().to_string(),
                message: error.to_string(),
            },
        )
    })?;

    Ok(SupportPaths {
        native_db_path: root_dir.join(NATIVE_DB_FILE),
        root_dir,
        logs_dir,
    })
}
