#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeAppConfig {
    pub(crate) provisioner_worker_image_ref: String,
    pub(crate) provisioner_worker_port: u16,
    pub(crate) runpod_endpoint_worker_image_ref: String,
    pub(crate) runpod_endpoint_worker_port: u16,
}

impl NativeAppConfig {
    pub(crate) fn from_build_environment() -> Self {
        Self {
            provisioner_worker_image_ref: env!("LUMA_FORGE_PROVISIONER_WORKER_IMAGE_REF")
                .to_string(),
            provisioner_worker_port: parse_build_port(env!("LUMA_FORGE_PROVISIONER_WORKER_PORT")),
            runpod_endpoint_worker_image_ref: env!("LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_IMAGE_REF")
                .to_string(),
            runpod_endpoint_worker_port: parse_build_port(env!(
                "LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT"
            )),
        }
    }
}

fn parse_build_port(value: &'static str) -> u16 {
    value
        .parse()
        .expect("worker build configuration port must be a valid u16")
}
