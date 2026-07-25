use luma_diagnostics::diagnostic;

#[derive(Clone)]
struct Service;

impl Service {
    #[diagnostic(detached)]
    async fn run(self) -> Result<(), ()> {
        Ok(())
    }
}

fn main() {
    let _future: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ()>> + Send>> =
        Box::pin(Service.run());
}
