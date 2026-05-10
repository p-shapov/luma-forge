//! Tauri command boundary exposed to the React client.

use command_error::NativeCommandError;

mod command_bindings;
mod command_builder;
mod command_error;
mod provider_contracts;
mod provider_setup;
mod workspace;

#[cfg(any(debug_assertions, test))]
pub(crate) use command_bindings::export_typescript_bindings;
pub(crate) use command_builder::builder;

type CommandResult<T> = Result<T, NativeCommandError>;
