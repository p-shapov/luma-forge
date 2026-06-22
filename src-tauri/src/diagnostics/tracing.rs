use fastrace::{collector::SpanContext, Span};

pub fn trace_id_from_context(context: &SpanContext) -> String {
    context.trace_id.to_string()
}

pub fn trace_id_from_span(span: &Span) -> Option<String> {
    SpanContext::from_span(span).map(|context| trace_id_from_context(&context))
}

pub fn current_trace_id() -> Option<String> {
    SpanContext::current_local_parent().map(|context| trace_id_from_context(&context))
}
