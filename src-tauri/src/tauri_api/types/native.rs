use serde::{Deserialize, Serialize};
use specta::Type;

use crate::tauri_api::errors::{CommandError, NativeInitializationCommandErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum NativeStartupStatusResponse {
    Ready,
    Failed {
        error: CommandError<NativeInitializationCommandErrorCode>,
    },
}
