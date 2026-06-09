use crate::domain::lifecycle_operation::LifecycleOperationId;

use super::coordination::LifecycleOperationRegistry;

pub async fn run(operation_id: LifecycleOperationId, registry: LifecycleOperationRegistry) {
    registry.complete(&operation_id);
}
