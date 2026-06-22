use std::future::Future;

use fastrace::{collector::SpanContext, future::FutureExt, Span};

use crate::{diagnostics, tauri_api::CommandResult};

const TRACE_UNAVAILABLE: &str = "trace-unavailable";

pub(crate) async fn run_async_command<T, Code, Fut>(
    name: &'static str,
    handler: impl FnOnce(String) -> Fut,
) -> CommandResult<T, Code>
where
    Fut: Future<Output = CommandResult<T, Code>>,
{
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());

    async move { handler(trace_id.clone()).await }
        .in_span(root)
        .await
}

pub(crate) fn run_sync_command<T, Code>(
    name: &'static str,
    handler: impl FnOnce(String) -> CommandResult<T, Code>,
) -> CommandResult<T, Code> {
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());
    let _guard = root.set_local_parent();

    handler(trace_id)
}
