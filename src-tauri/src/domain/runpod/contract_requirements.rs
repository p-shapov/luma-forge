use serde::{Deserialize, Serialize};

use crate::domain::runtime_contract::RuntimeContractReference;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodContractRequirements {
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
}
