use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workflow_id: String,
    pub workflow_version: String,
    pub state: String,
    pub runtime_kind: String,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    #[sea_orm(has_many)]
    pub lifecycle_operations: HasMany<super::lifecycle_operations::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
