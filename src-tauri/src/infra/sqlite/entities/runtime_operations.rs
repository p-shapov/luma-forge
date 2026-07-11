use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "runtime_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workspace_id: String,
    #[sea_orm(unique)]
    pub running_workspace_id: Option<String>,
    pub operation_kind: String,
    pub state: String,
    pub trace_id: String,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub finished_at: Option<TimeDateTimeWithTimeZone>,
}

impl ActiveModelBehavior for ActiveModel {}
