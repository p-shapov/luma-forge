use serde::{Deserialize, Serialize};

use crate::application::runtimes::runpod::RunpodRuntime;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Runpod,
}

impl RuntimeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

impl std::str::FromStr for RuntimeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "runpod" => Ok(Self::Runpod),
            _ => Err(()),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "payload", deny_unknown_fields)]
pub enum RuntimeProvider {
    #[serde(rename = "runpod")]
    Runpod(#[diagnostic(show)] RunpodRuntime),
}

impl RuntimeProvider {
    pub fn kind(&self) -> RuntimeKind {
        match self {
            Self::Runpod(_) => RuntimeKind::Runpod,
        }
    }

    pub fn as_runpod(&self) -> Option<&RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }

    pub fn as_runpod_mut(&mut self) -> Option<&mut RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Runtime {
    #[diagnostic(show)]
    pub state: RuntimeState,
    #[diagnostic(show)]
    pub provider: RuntimeProvider,
}

impl Runtime {
    pub fn kind(&self) -> RuntimeKind {
        self.provider.kind()
    }
}

#[cfg(test)]
pub(super) fn provider_payload_fixture() -> RuntimeProvider {
    use crate::application::runtimes::runpod::{RunpodRuntime, RunpodRuntimeConfig};
    use uuid::Uuid;

    let mut runtime = RunpodRuntime::new_provisioning(
        Uuid::from_u128(1),
        RunpodRuntimeConfig {
            datacenter_id: "EU-RO-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb: 100,
        },
    );
    runtime.resources.network_volume_id = Some("network-volume-1".into());
    runtime.resources.template_id = Some("template-1".into());
    RuntimeProvider::Runpod(runtime)
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::runpod::{RunpodRuntime, RunpodRuntimeConfig};

    use super::*;

    #[test]
    fn runtime_kind_uses_the_pinned_neutral_identifier() {
        assert_eq!(RuntimeKind::Runpod.as_str(), "runpod");
        assert_eq!("runpod".parse::<RuntimeKind>(), Ok(RuntimeKind::Runpod));
        assert_eq!("Runpod".parse::<RuntimeKind>(), Err(()));
    }

    #[test]
    fn provider_payload_is_tagged_round_trippable_and_strict() {
        let provider = provider_payload_fixture();
        let value = serde_json::to_value(&provider).unwrap();

        assert_eq!(value["provider"], "runpod");
        assert_eq!(
            value["payload"]["provision_operation_id"],
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(value["payload"]["config"]["datacenter_id"], "EU-RO-1");
        assert_eq!(value["payload"]["resources"]["template_id"], "template-1");
        assert_eq!(
            serde_json::from_value::<RuntimeProvider>(value.clone()).unwrap(),
            provider
        );

        let mut unknown_field = value.clone();
        unknown_field["payload"]["config"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RuntimeProvider>(unknown_field).is_err());

        let mut missing_provision_operation_id = value.clone();
        missing_provision_operation_id["payload"]
            .as_object_mut()
            .unwrap()
            .remove("provision_operation_id");
        assert!(serde_json::from_value::<RuntimeProvider>(missing_provision_operation_id).is_err());

        let mut invalid_type = value;
        invalid_type["payload"]["config"]["volume_size_gb"] = serde_json::json!("100");
        assert!(serde_json::from_value::<RuntimeProvider>(invalid_type).is_err());
        assert!(
            serde_json::from_value::<RuntimeProvider>(serde_json::json!({
                "provider": "unknown",
                "payload": {}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<RuntimeProvider>(serde_json::json!({
                "provider": "runpod",
                "config": {}
            }))
            .is_err()
        );
    }

    #[test]
    fn runtime_kind_comes_from_its_provider() {
        let runtime = Runtime {
            state: RuntimeState::Provisioning,
            provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
                uuid::Uuid::from_u128(1),
                RunpodRuntimeConfig {
                    datacenter_id: "EU-RO-1".into(),
                    gpu_id: "gpu-1".into(),
                    volume_size_gb: 100,
                },
            )),
        };

        assert_eq!(runtime.kind(), RuntimeKind::Runpod);
    }

    #[test]
    fn runtime_provider_exposes_its_runpod_value() {
        let mut provider = RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
            uuid::Uuid::from_u128(1),
            RunpodRuntimeConfig {
                datacenter_id: "EU-RO-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 100,
            },
        ));

        assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 100);
        provider.as_runpod_mut().unwrap().config.volume_size_gb = 120;
        assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 120);
    }
}
