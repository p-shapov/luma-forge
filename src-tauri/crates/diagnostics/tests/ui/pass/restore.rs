use luma_diagnostics::__private::uuid::Uuid;
use luma_diagnostics::diagnostic;

#[diagnostic(restore = trace_id)]
async fn restore(trace_id: Option<Uuid>) -> Result<(), ()> {
    Ok(())
}

struct Operation {
    trace_id: Option<Uuid>,
}

#[diagnostic(restore = operation.trace_id)]
async fn restore_from_field(operation: Operation) -> Result<(), ()> {
    std::mem::drop(operation);
    Ok(())
}

fn main() {
    std::mem::drop(restore(Some(Uuid::new_v4())));
    std::mem::drop(restore(None));
    std::mem::drop(restore_from_field(Operation { trace_id: None }));
}
