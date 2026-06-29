use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workflow_id: String,
    pub workflow_version: String,
    pub state: String,
    pub runtime_kind: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::runpod_workspace_runtimes::Entity")]
    RunpodWorkspaceRuntime,
    #[sea_orm(has_many = "super::lifecycle_operations::Entity")]
    LifecycleOperation,
}

impl Related<super::runpod_workspace_runtimes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RunpodWorkspaceRuntime.def()
    }
}

impl Related<super::lifecycle_operations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifecycleOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
