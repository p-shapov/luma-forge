pub fn new_diagnostic_id() -> String {
    format!("diag-{}", uuid::Uuid::new_v4())
}
