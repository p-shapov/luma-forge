use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspace_runtimes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    pub provider_kind: String,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id", on_delete = "Cascade")]
    pub workspace: HasOne<super::workspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::runpod_workspace_runtimes::Entity> for Entity {
    fn to() -> RelationDef {
        super::runpod_workspace_runtimes::Relation::WorkspaceRuntimes
            .def()
            .rev()
    }
}
