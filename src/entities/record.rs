use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "issue_record")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub cert_id: i32,
    /// 证书备注名快照
    pub cert_name: String,
    /// manual | auto
    pub trigger_type: String,
    /// 颁发时间（完成时间）
    pub issued_at: String,
    /// 证书到期时间
    pub expire_at: Option<String>,
    pub provider_type: String,
    pub provider_name: String,
    /// JSON 数组快照
    pub domains: String,
    pub cert_path: String,
    pub key_path: String,
    pub ca: String,
    /// 1 成功 / 0 失败
    pub success: i32,
    pub message: String,
    /// acme.sh 与完成后命令的完整输出
    pub command_output: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
