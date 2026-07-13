use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "runpod_workspace_runtimes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: i64,
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
    #[sea_orm(
        belongs_to,
        from = "workspace_id",
        to = "workspace_id",
        on_delete = "Cascade"
    )]
    pub workspace_runtime: HasOne<super::workspace_runtimes::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
