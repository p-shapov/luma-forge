use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "runpod_operation_payloads")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub operation_id: String,
    pub step: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::lifecycle_operations::Entity",
        from = "Column::OperationId",
        to = "super::lifecycle_operations::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    LifecycleOperation,
}

impl Related<super::lifecycle_operations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifecycleOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
