use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "cert")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 备注名，可为空
    pub name: String,
    pub provider_id: i32,
    /// JSON 数组: ["test.example.com", "*.example.com"]
    pub domains: String,
    /// 公钥位置（证书文件绝对路径）
    pub cert_path: String,
    /// 私钥位置（密钥文件绝对路径）
    pub key_path: String,
    /// CA 机构展示名
    pub ca: String,
    /// 续期周期（天）
    pub renewal_days: i32,
    /// 颁发完成后执行的命令（shell 或 cmd）
    pub post_command: String,
    /// pending | issuing | issued | failed
    pub status: String,
    pub last_issued_at: Option<String>,
    pub cert_expire_at: Option<String>,
    pub last_error: String,
    /// 连续失败次数（用于自动重试退避）
    pub fail_count: i32,
    pub last_attempt_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
