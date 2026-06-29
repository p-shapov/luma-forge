use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Workspaces::Table)
                    .col(
                        ColumnDef::new(Workspaces::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Workspaces::WorkflowId).string().not_null())
                    .col(
                        ColumnDef::new(Workspaces::WorkflowVersion)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Workspaces::State).string().not_null())
                    .col(ColumnDef::new(Workspaces::RuntimeKind).string().not_null())
                    .col(ColumnDef::new(Workspaces::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Workspaces::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RunpodWorkspaceRuntimes::Table)
                    .col(
                        ColumnDef::new(RunpodWorkspaceRuntimes::WorkspaceId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RunpodWorkspaceRuntimes::DatacenterId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RunpodWorkspaceRuntimes::GpuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RunpodWorkspaceRuntimes::VolumeSizeGb)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RunpodWorkspaceRuntimes::NetworkVolumeId).string())
                    .col(ColumnDef::new(RunpodWorkspaceRuntimes::ProvisionerPodId).string())
                    .col(ColumnDef::new(RunpodWorkspaceRuntimes::EndpointId).string())
                    .col(ColumnDef::new(RunpodWorkspaceRuntimes::TemplateId).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_runpod_workspace_runtimes_workspace_id")
                            .from(
                                RunpodWorkspaceRuntimes::Table,
                                RunpodWorkspaceRuntimes::WorkspaceId,
                            )
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LifecycleOperations::Table)
                    .col(
                        ColumnDef::new(LifecycleOperations::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(LifecycleOperations::WorkspaceId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LifecycleOperations::OperationKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LifecycleOperations::State)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LifecycleOperations::CreatedAt)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LifecycleOperations::UpdatedAt)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(LifecycleOperations::FinishedAt).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_lifecycle_operations_workspace_id")
                            .from(LifecycleOperations::Table, LifecycleOperations::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RunpodOperationPayloads::Table)
                    .col(
                        ColumnDef::new(RunpodOperationPayloads::OperationId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RunpodOperationPayloads::Step)
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_runpod_operation_payloads_operation_id")
                            .from(
                                RunpodOperationPayloads::Table,
                                RunpodOperationPayloads::OperationId,
                            )
                            .to(LifecycleOperations::Table, LifecycleOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RunpodOperationPayloads::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(LifecycleOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RunpodWorkspaceRuntimes::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Workspaces::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Workspaces {
    Table,
    Id,
    WorkflowId,
    WorkflowVersion,
    State,
    RuntimeKind,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RunpodWorkspaceRuntimes {
    Table,
    WorkspaceId,
    DatacenterId,
    GpuId,
    VolumeSizeGb,
    NetworkVolumeId,
    ProvisionerPodId,
    EndpointId,
    TemplateId,
}

#[derive(DeriveIden)]
enum LifecycleOperations {
    Table,
    Id,
    WorkspaceId,
    OperationKind,
    State,
    CreatedAt,
    UpdatedAt,
    FinishedAt,
}

#[derive(DeriveIden)]
enum RunpodOperationPayloads {
    Table,
    OperationId,
    Step,
}
