mod runpod;

use serde_json::{Map, Value};

use crate::domain::workflow_preset::WorkflowContractRequirements;

use super::WorkflowCatalogError;

const RUNTIME_TYPE_FIELD: &str = "runtime_type";

#[cfg(test)]
pub fn encode_contract_requirements(
    requirements: &[WorkflowContractRequirements],
) -> Result<Value, WorkflowCatalogError> {
    requirements
        .iter()
        .map(encode_one_contract_requirements)
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

pub fn decode_contract_requirements(
    values: Vec<Value>,
) -> Result<Vec<WorkflowContractRequirements>, WorkflowCatalogError> {
    values
        .into_iter()
        .map(decode_one_contract_requirements)
        .collect()
}

#[cfg(test)]
fn encode_one_contract_requirements(
    requirements: &WorkflowContractRequirements,
) -> Result<Value, WorkflowCatalogError> {
    match requirements {
        WorkflowContractRequirements::Runpod(requirements) => runpod::encode(requirements),
    }
}

fn decode_one_contract_requirements(
    value: Value,
) -> Result<WorkflowContractRequirements, WorkflowCatalogError> {
    let runtime_type = value
        .get(RUNTIME_TYPE_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| parse_failed_message("contract requirements runtime type is missing"))?;

    match runtime_type {
        runpod::RUNTIME_TYPE => runpod::decode(value),
        runtime_type => Err(parse_failed_message(format!(
            "unknown contract requirements runtime type: {runtime_type}"
        ))),
    }
}

#[cfg(test)]
fn with_runtime_type(runtime_type: &str, value: Value) -> Result<Value, WorkflowCatalogError> {
    let Value::Object(mut object) = value else {
        return Err(parse_failed_message(
            "contract requirements did not encode as an object",
        ));
    };

    object.insert(
        RUNTIME_TYPE_FIELD.to_string(),
        Value::String(runtime_type.to_string()),
    );

    Ok(Value::Object(object))
}

fn without_runtime_type(mut value: Value) -> Result<Value, WorkflowCatalogError> {
    let Value::Object(ref mut object) = value else {
        return Err(parse_failed_message(
            "contract requirements must be an object",
        ));
    };

    object.remove(RUNTIME_TYPE_FIELD);
    Ok(value)
}

fn parse_failed_message(message: impl Into<String>) -> WorkflowCatalogError {
    WorkflowCatalogError::ParseFailed {
        message: message.into(),
    }
}

fn object_from_value(value: Value) -> Result<Map<String, Value>, WorkflowCatalogError> {
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(parse_failed_message(
            "contract requirements must be an object",
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        runpod::RunpodContractRequirements, runtime_contract::RuntimeContractReference,
        workflow_preset::WorkflowContractRequirements,
    };

    use super::*;

    fn runpod_requirements() -> WorkflowContractRequirements {
        WorkflowContractRequirements::Runpod(RunpodContractRequirements {
            endpoint_contract: RuntimeContractReference {
                id: "endpoint".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: RuntimeContractReference {
                id: "provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
        })
    }

    #[test]
    fn runpod_contract_requirements_decode_from_runtime_typed_json() {
        let value = serde_json::json!({
            "runtime_type": "runpod",
            "endpoint_contract": {
                "id": "endpoint",
                "version": "1.0.0"
            },
            "provisioner_contract": {
                "id": "provisioner",
                "version": "1.0.0"
            }
        });

        assert_eq!(
            decode_contract_requirements(vec![value]).expect("requirements should decode"),
            vec![runpod_requirements()]
        );
    }

    #[test]
    fn unknown_runtime_type_is_rejected() {
        let value = serde_json::json!({
            "runtime_type": "unknown",
            "endpoint_contract": {
                "id": "endpoint",
                "version": "1.0.0"
            },
            "provisioner_contract": {
                "id": "provisioner",
                "version": "1.0.0"
            }
        });

        assert_eq!(
            decode_contract_requirements(vec![value]),
            Err(WorkflowCatalogError::ParseFailed {
                message: "unknown contract requirements runtime type: unknown".to_string()
            })
        );
    }

    #[test]
    fn missing_runtime_type_is_rejected() {
        let value = serde_json::json!({
            "endpoint_contract": {
                "id": "endpoint",
                "version": "1.0.0"
            },
            "provisioner_contract": {
                "id": "provisioner",
                "version": "1.0.0"
            }
        });

        assert_eq!(
            decode_contract_requirements(vec![value]),
            Err(WorkflowCatalogError::ParseFailed {
                message: "contract requirements runtime type is missing".to_string()
            })
        );
    }

    #[test]
    fn runpod_contract_requirements_encode_to_runtime_typed_json() {
        let encoded = encode_contract_requirements(&[runpod_requirements()])
            .expect("requirements should encode");

        assert_eq!(
            encoded,
            serde_json::json!([
                {
                    "runtime_type": "runpod",
                    "endpoint_contract": {
                        "id": "endpoint",
                        "version": "1.0.0"
                    },
                    "provisioner_contract": {
                        "id": "provisioner",
                        "version": "1.0.0"
                    }
                }
            ])
        );
    }
}
