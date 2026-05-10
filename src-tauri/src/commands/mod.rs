//! Tauri command boundary exposed to the React client.

use error::NativeCommandError;

mod bindings;
mod builder;
mod contracts;
mod error;
mod provider_setup;
mod workspace;

#[cfg(any(debug_assertions, test))]
pub(crate) use bindings::export_typescript_bindings;
pub(crate) use builder::builder;

type CommandResult<T> = Result<T, NativeCommandError>;
