use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workflow_id: String,
    pub workflow_revision: String,
    pub created_at: TimeDateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::workspace_runtimes::Entity> for Entity {
    fn to() -> RelationDef {
        super::workspace_runtimes::Relation::Workspaces.def().rev()
    }
}
