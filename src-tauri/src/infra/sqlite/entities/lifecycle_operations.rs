use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "lifecycle_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workspace_id: String,
    pub operation_kind: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id", on_delete = "Cascade")]
    pub workspace: HasOne<super::workspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
