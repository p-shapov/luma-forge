mod parser;

use std::path::Path;

use parser::{AppConfigParseError, BuildEnvironment, NonEmptyEnvValue};

const PROVISIONER_WORKER_PORT_ENV: &str = "LUMA_FORGE_PROVISIONER_WORKER_PORT";
const RUNPOD_ENDPOINT_WORKER_PORT_ENV: &str = "LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT";

pub(crate) struct AppConfig {
    provisioner_worker_port: NonEmptyEnvValue,
    runpod_endpoint_worker_port: NonEmptyEnvValue,
}

impl AppConfig {
    pub(crate) fn from_build_environment() -> Result<Self, AppConfigParseError> {
        let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri should live under the workspace root");
        let dotenv_path = workspace_dir.join(".env");
        let source = BuildEnvironment::new(&dotenv_path)?;

        source.emit_cargo_rerun_instructions(&[
            PROVISIONER_WORKER_PORT_ENV,
            RUNPOD_ENDPOINT_WORKER_PORT_ENV,
        ]);

        Ok(Self {
            provisioner_worker_port: source.parse_non_empty(PROVISIONER_WORKER_PORT_ENV)?,
            runpod_endpoint_worker_port: source.parse_non_empty(RUNPOD_ENDPOINT_WORKER_PORT_ENV)?,
        })
    }

    pub(crate) fn emit_cargo_env(&self) {
        emit_env(
            PROVISIONER_WORKER_PORT_ENV,
            self.provisioner_worker_port.as_str(),
        );
        emit_env(
            RUNPOD_ENDPOINT_WORKER_PORT_ENV,
            self.runpod_endpoint_worker_port.as_str(),
        );
    }
}

fn emit_env(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}
