use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SupportPaths {
    root_dir: PathBuf,
    logs_dir: PathBuf,
    native_db_path: PathBuf,
}

impl SupportPaths {
    pub fn new(root_dir: PathBuf, logs_dir: PathBuf, native_db_path: PathBuf) -> Self {
        Self {
            root_dir,
            logs_dir,
            native_db_path,
        }
    }

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
