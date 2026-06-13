use serde::{Deserialize, Serialize};
use specta::Type;

use crate::commands::errors::NativeCommandError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum NativeStartupStatusResponse {
    Ready,
    Failed { error: NativeCommandError },
}
