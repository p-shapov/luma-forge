use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::domain::lifecycle_operation::LifecycleOperationId;

#[derive(Clone, Default)]
pub struct LifecycleOperationRegistry {
    operation_ids: Arc<Mutex<HashSet<LifecycleOperationId>>>,
}

impl LifecycleOperationRegistry {
    pub fn try_register(&self, operation_id: &LifecycleOperationId) -> bool {
        self.operation_ids
            .lock()
            .expect("lifecycle operation registry lock should succeed")
            .insert(operation_id.clone())
    }

    pub fn complete(&self, operation_id: &LifecycleOperationId) {
        self.operation_ids
            .lock()
            .expect("lifecycle operation registry lock should succeed")
            .remove(operation_id);
    }
}
