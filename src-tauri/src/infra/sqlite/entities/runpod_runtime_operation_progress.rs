use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "runpod_runtime_operation_progress")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub operation_id: String,
    pub step: String,
    #[sea_orm(belongs_to, from = "operation_id", to = "id", on_delete = "Cascade")]
    pub runtime_operation: HasOne<super::runtime_operations::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
