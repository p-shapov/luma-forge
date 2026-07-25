use luma_diagnostics::diagnostic;

#[derive(Debug)]
struct SafeError;
impl luma_diagnostics::DiagnosticValue for SafeError {}

#[async_trait::async_trait]
trait Port {
    async fn get(&self, id: String) -> Result<String, SafeError>;
}

struct Adapter;

#[diagnostic]
#[async_trait::async_trait]
impl Port for Adapter {
    #[diagnostic(show_output, show_error)]
    async fn get(&self, #[diagnostic(show)] id: String) -> Result<String, SafeError> {
        Ok(id)
    }
}

fn main() {}
