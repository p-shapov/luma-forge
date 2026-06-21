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
                trace_id = error.trace_id.as_str(),
                message = error.message.as_str();
                "tauri command failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use crate::{
        diagnostics,
        tauri_api::{
            errors::{command_error, CommandError},
            tracing::{run_async_command, run_sync_command},
            CommandResult,
        },
    };

    #[test]
    fn run_sync_command_passes_root_span_trace_id_to_errors() {
        let mut observed_trace_id = None;
        let result: CommandResult<(), &'static str> = run_sync_command("test.sync", |trace_id| {
            let current_trace_id = diagnostics::current_trace_id()
                .expect("sync command wrapper should set a local parent span");
            assert_eq!(trace_id, current_trace_id);
            observed_trace_id = Some(trace_id.clone());

            Err(command_error(
                &trace_id,
                io::Error::other("sync failure"),
                |_| "sync_error",
            ))
        });

        let error = result.expect_err("sync wrapper should propagate command errors");
        assert_eq!(
            error.trace_id,
            observed_trace_id.expect("sync command handler should observe a trace id")
        );
    }

    #[tokio::test]
    async fn run_async_command_passes_root_span_trace_id_to_errors() {
        let result: CommandResult<(), &'static str> =
            run_async_command("test.async", |trace_id| async move {
                let current_trace_id = diagnostics::current_trace_id()
                    .expect("async command wrapper should set a local parent span");
                assert_eq!(trace_id, current_trace_id);

                Err(command_error(
                    &trace_id,
                    io::Error::other("async failure"),
                    |_| "async_error",
                ))
            })
            .await;

        let error = result.expect_err("async wrapper should propagate command errors");
        assert_ne!(error.trace_id, "trace-unavailable");
    }

    #[test]
    fn command_error_from_code_uses_non_command_fallback_trace_id() {
        let error: CommandError<&'static str> = "fallback_error".into();

        assert_eq!(error.trace_id, "trace-unavailable");
    }
}
