mod parser;

use std::path::Path;

use parser::{AppConfigParseError, BuildEnvironment, NonEmptyEnvValue};

const PROVISIONER_WORKER_IMAGE_REF_ENV: &str = "LUMA_FORGE_PROVISIONER_WORKER_IMAGE_REF";
const PROVISIONER_WORKER_PORT_ENV: &str = "LUMA_FORGE_PROVISIONER_WORKER_PORT";
const ENDPOINT_WORKER_IMAGE_REF_ENV: &str = "LUMA_FORGE_ENDPOINT_WORKER_IMAGE_REF";
const ENDPOINT_WORKER_PORT_ENV: &str = "LUMA_FORGE_ENDPOINT_WORKER_PORT";

pub(crate) struct AppConfig {
    provisioner_worker_image_ref: NonEmptyEnvValue,
    provisioner_worker_port: NonEmptyEnvValue,
    endpoint_worker_image_ref: NonEmptyEnvValue,
    endpoint_worker_port: NonEmptyEnvValue,
}

impl AppConfig {
    pub(crate) fn from_build_environment() -> Result<Self, AppConfigParseError> {
        let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri should live under the workspace root");
        let dotenv_path = workspace_dir.join(".env");
        let source = BuildEnvironment::new(&dotenv_path)?;

        source.emit_cargo_rerun_instructions(&[
            PROVISIONER_WORKER_IMAGE_REF_ENV,
            PROVISIONER_WORKER_PORT_ENV,
            ENDPOINT_WORKER_IMAGE_REF_ENV,
            ENDPOINT_WORKER_PORT_ENV,
        ]);

        Ok(Self {
            provisioner_worker_image_ref: source
                .parse_non_empty(PROVISIONER_WORKER_IMAGE_REF_ENV)?,
            provisioner_worker_port: source.parse_non_empty(PROVISIONER_WORKER_PORT_ENV)?,
            endpoint_worker_image_ref: source.parse_non_empty(ENDPOINT_WORKER_IMAGE_REF_ENV)?,
            endpoint_worker_port: source.parse_non_empty(ENDPOINT_WORKER_PORT_ENV)?,
        })
    }

    pub(crate) fn emit_cargo_env(&self) {
        emit_env(
            PROVISIONER_WORKER_IMAGE_REF_ENV,
            self.provisioner_worker_image_ref.as_str(),
        );
        emit_env(
            PROVISIONER_WORKER_PORT_ENV,
            self.provisioner_worker_port.as_str(),
        );
        emit_env(
            ENDPOINT_WORKER_IMAGE_REF_ENV,
            self.endpoint_worker_image_ref.as_str(),
        );
        emit_env(ENDPOINT_WORKER_PORT_ENV, self.endpoint_worker_port.as_str());
    }
}

fn emit_env(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}
