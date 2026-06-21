use std::{fmt::Debug, future::Future};

use fastrace::{collector::SpanContext, future::FutureExt, Span};

use crate::{diagnostics, tauri_api::CommandResult};

const TRACE_UNAVAILABLE: &str = "trace-unavailable";

pub(crate) async fn run_async_command<T, Code, Fut>(
    name: &'static str,
    handler: impl FnOnce(String) -> Fut,
) -> CommandResult<T, Code>
where
    Code: Debug,
    Fut: Future<Output = CommandResult<T, Code>>,
{
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());

    async move {
        log::info!(command = name; "tauri command started");
        let result = handler(trace_id.clone()).await;
        log_command_result(name, &result);
        result
    }
    .in_span(root)
    .await
}

pub(crate) fn run_sync_command<T, Code>(
    name: &'static str,
    handler: impl FnOnce(String) -> CommandResult<T, Code>,
) -> CommandResult<T, Code>
where
    Code: Debug,
{
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());
    let _guard = root.set_local_parent();

    log::info!(command = name; "tauri command started");
    let result = handler(trace_id);
    log_command_result(name, &result);
    result
}

fn log_command_result<T, Code>(name: &'static str, result: &CommandResult<T, Code>)
where
    Code: Debug,
{
    match result {
        Ok(_) => log::info!(command = name; "tauri command completed"),
        Err(error) => {
            log::error!(
                command = name,
                code:? = &error.code,
                trace_id = error.trace_id.as_str();
                "tauri command failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {}
