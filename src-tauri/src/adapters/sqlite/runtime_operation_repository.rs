use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use crate::{
    application::runtimes::{
        ports::{RuntimeOperationRepository, RuntimeOperationRepositoryError},
        RuntimeKind, RuntimeOperation, RuntimeOperationKind, RuntimeOperationState,
        RuntimeProgress,
    },
    infra::sqlite::entities::runtime_operations,
};

pub struct SqliteRuntimeOperationRepository {
    connection: DatabaseConnection,
}

impl SqliteRuntimeOperationRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    async fn load(
        &self,
        query: sea_orm::Select<runtime_operations::Entity>,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        query
            .all(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
            .into_iter()
            .map(map_operation)
            .collect()
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl RuntimeOperationRepository for SqliteRuntimeOperationRepository {
    #[diagnostic(show_output, show_error)]
    async fn page(
        &self,
        #[diagnostic(show)] workspace_id: Option<&str>,
        #[diagnostic(show)] offset: u64,
        #[diagnostic(show)] limit: u64,
    ) -> Result<(Vec<RuntimeOperation>, u64), RuntimeOperationRepositoryError> {
        let query = match workspace_id {
            Some(workspace_id) => runtime_operations::Entity::find()
                .filter(runtime_operations::Column::WorkspaceId.eq(workspace_id)),
            None => runtime_operations::Entity::find(),
        };
        let total = query
            .clone()
            .count(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?;
        let operations = self
            .load(
                query
                    .order_by_desc(runtime_operations::Column::CreatedAt)
                    .order_by_desc(runtime_operations::Column::Id)
                    .offset(offset)
                    .limit(limit),
            )
            .await?;
        Ok((operations, total))
    }

    #[diagnostic(show_output, show_error)]
    async fn running(&self) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.load(
            runtime_operations::Entity::find()
                .filter(runtime_operations::Column::State.eq("running")),
        )
        .await
    }

    #[diagnostic(show_output, show_error)]
    async fn has_running(
        &self,
        #[diagnostic(show)] workspace_id: &str,
    ) -> Result<bool, RuntimeOperationRepositoryError> {
        Ok(runtime_operations::Entity::find()
            .filter(runtime_operations::Column::WorkspaceId.eq(workspace_id))
            .filter(runtime_operations::Column::State.eq("running"))
            .one(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
            .is_some())
    }
}

fn map_operation(
    model: runtime_operations::Model,
) -> Result<RuntimeOperation, RuntimeOperationRepositoryError> {
    let runtime_kind = parse_runtime_kind(&model.runtime_kind)?;
    let kind = match model.operation_kind.as_str() {
        "provision" => RuntimeOperationKind::Provision,
        "cleanup" => RuntimeOperationKind::Cleanup,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let state = match model.state.as_str() {
        "running" => RuntimeOperationState::Running,
        "succeeded" => RuntimeOperationState::Succeeded,
        "failed" => RuntimeOperationState::Failed,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let progress = serde_json::from_str::<RuntimeProgress>(&model.progress_payload)
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)?;
    let operation = RuntimeOperation {
        id: Uuid::parse_str(&model.id).map_err(|_| RuntimeOperationRepositoryError::CorruptData)?,
        workspace_id: model.workspace_id,
        runtime_kind,
        kind,
        state,
        trace_id: model
            .trace_id
            .map(|trace_id| {
                Uuid::parse_str(&trace_id).map_err(|_| RuntimeOperationRepositoryError::CorruptData)
            })
            .transpose()?,
        progress,
        created_at: model.created_at,
        updated_at: model.updated_at,
        finished_at: model.finished_at,
    };
    operation
        .validate_progress()
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)?;
    Ok(operation)
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, RuntimeOperationRepositoryError> {
    value
        .parse()
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)
}

pub(super) fn runtime_operation_kind_value(kind: RuntimeOperationKind) -> &'static str {
    match kind {
        RuntimeOperationKind::Provision => "provision",
        RuntimeOperationKind::Cleanup => "cleanup",
    }
}

pub(super) fn runtime_operation_state_value(state: RuntimeOperationState) -> &'static str {
    match state {
        RuntimeOperationState::Running => "running",
        RuntimeOperationState::Succeeded => "succeeded",
        RuntimeOperationState::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(trace_id: Option<&str>) -> runtime_operations::Model {
        runtime_operations::Model {
            id: Uuid::nil().to_string(),
            workspace_id: "workspace-1".into(),
            runtime_kind: RuntimeKind::Runpod.as_str().into(),
            operation_kind: "provision".into(),
            state: "running".into(),
            trace_id: trace_id.map(str::to_owned),
            progress_payload: serde_json::to_string(
                &crate::application::runtimes::progress_fixture(),
            )
            .unwrap(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        }
    }

    #[test]
    fn trace_mapping_accepts_uuid_or_null_and_rejects_invalid_text() {
        let trace_id = Uuid::new_v4();
        assert_eq!(
            map_operation(model(Some(&trace_id.to_string())))
                .unwrap()
                .trace_id,
            Some(trace_id)
        );
        assert_eq!(map_operation(model(None)).unwrap().trace_id, None);
        assert_eq!(
            map_operation(model(Some("invalid"))),
            Err(RuntimeOperationRepositoryError::CorruptData)
        );
    }
}
