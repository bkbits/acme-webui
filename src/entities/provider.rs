use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "provider")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 配置名称（= DNS 运营商中文名，由后端根据 provider_type 确定，不由用户自定义）
    pub name: String,
    /// 权威英文名（注册表 key，用于匹配 acme.sh --dns 插件）
    pub provider_type: String,
    /// JSON: { 环境变量名: 值 }
    pub credentials: String,
    pub enabled: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
