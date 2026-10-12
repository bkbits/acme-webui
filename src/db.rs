use crate::entities::{cert, provider, user};
use crate::registry;
use sea_orm::sea_query::Expr;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

const DDL: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS user (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        username TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        created_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS provider (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        provider_type TEXT NOT NULL,
        credentials TEXT NOT NULL DEFAULT '{}',
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS cert (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL DEFAULT '',
        provider_id INTEGER NOT NULL,
        domains TEXT NOT NULL DEFAULT '[]',
        cert_path TEXT NOT NULL,
        key_path TEXT NOT NULL,
        ca TEXT NOT NULL DEFAULT 'Let''s Encrypt',
        renewal_days INTEGER NOT NULL DEFAULT 60,
        post_command TEXT NOT NULL DEFAULT '',
        status TEXT NOT NULL DEFAULT 'pending',
        last_issued_at TEXT,
        cert_expire_at TEXT,
        last_error TEXT NOT NULL DEFAULT '',
        fail_count INTEGER NOT NULL DEFAULT 0,
        last_attempt_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_cert_status ON cert(status)",
    "CREATE TABLE IF NOT EXISTS issue_record (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        cert_id INTEGER NOT NULL,
        cert_name TEXT NOT NULL DEFAULT '',
        trigger_type TEXT NOT NULL DEFAULT 'manual',
        issued_at TEXT NOT NULL,
        expire_at TEXT,
        provider_type TEXT NOT NULL,
        provider_name TEXT NOT NULL DEFAULT '',
        domains TEXT NOT NULL DEFAULT '[]',
        cert_path TEXT NOT NULL,
        key_path TEXT NOT NULL,
        ca TEXT NOT NULL,
        success INTEGER NOT NULL,
        message TEXT NOT NULL DEFAULT '',
        command_output TEXT NOT NULL DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_record_cert ON issue_record(cert_id)",
    "CREATE INDEX IF NOT EXISTS idx_record_issued ON issue_record(issued_at)",
];

/// 初始化 SQLite 数据库连接并建表
pub async fn init(path: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let url = format!("sqlite://{path}?mode=rwc");
    let mut opt = ConnectOptions::new(&url);
    // 单连接串行化访问，避免 sqlite 文件锁冲突
    opt.max_connections(1).sqlx_logging(false);
    let conn = Database::connect(opt).await?;
    conn.execute_unprepared("PRAGMA journal_mode=WAL;").await?;
    conn.execute_unprepared("PRAGMA busy_timeout=5000;").await?;
    for sql in DDL {
        conn.execute_unprepared(sql).await?;
    }
    // 旧版运营商类型迁移 + 同名去重（先于唯一索引执行）
    migrate_provider_types(&conn).await?;
    conn.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_type ON provider(provider_type)",
    )
    .await?;
    Ok(conn)
}

/// 将旧版 provider_type（aliyun/cloudflare/huawei）迁移为权威英文名，
/// 配置名称统一为运营商中文名；同类型重复配置只保留 id 最小的一条
pub async fn migrate_provider_types(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    const LEGACY: &[(&str, &str)] = &[
        ("aliyun", "ali"),
        ("cloudflare", "cf"),
        ("huawei", "huaweicloud"),
    ];
    for (old, new) in LEGACY {
        if let Some(t) = registry::find_provider_type(new) {
            conn.execute_unprepared(&format!(
                "UPDATE provider SET provider_type = '{}', name = '{}' WHERE provider_type = '{}'",
                new, t.label, old
            ))
            .await?;
        }
    }
    // 同类型重复: 关联证书改指向保留行，删除其余行
    let rows = conn
        .query_all(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT p.id, (SELECT MIN(p2.id) FROM provider p2 WHERE p2.provider_type = p.provider_type) AS keep_id \
             FROM provider p \
             WHERE p.id > (SELECT MIN(p2.id) FROM provider p2 WHERE p2.provider_type = p.provider_type)",
        ))
        .await?;
    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let keep_id: i32 = row.try_get("", "keep_id")?;
        cert::Entity::update_many()
            .col_expr(cert::Column::ProviderId, Expr::value(keep_id))
            .filter(cert::Column::ProviderId.eq(id))
            .exec(conn)
            .await?;
        provider::Entity::delete_by_id(id).exec(conn).await?;
        tracing::warn!(
            "运营商配置重复（type 迁移），已合并保留 #{}，删除 #{}",
            keep_id,
            id
        );
    }
    Ok(())
}

/// 服务重启时，将中断的颁发任务置为 failed（到期未变则会自动重试）
pub async fn reset_issuing(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    cert::Entity::update_many()
        .col_expr(cert::Column::Status, Expr::value("failed"))
        .col_expr(
            cert::Column::LastError,
            Expr::value("服务重启，颁发任务已中断"),
        )
        .filter(cert::Column::Status.eq("issuing"))
        .exec(conn)
        .await?;
    Ok(())
}

/// 首次启动时创建默认管理员 admin（密码可用 ACME_WEBUI_ADMIN_PASSWORD 指定，默认 admin123）
pub async fn seed_admin(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    if user::Entity::find().one(conn).await?.is_some() {
        return Ok(());
    }
    let password =
        std::env::var("ACME_WEBUI_ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".to_string());
    let am = user::ActiveModel {
        username: Set("admin".to_string()),
        password_hash: Set(crate::auth::hash_password(&password)),
        created_at: Set(crate::util::now_str()),
        ..Default::default()
    };
    am.insert(conn).await?;
    tracing::info!("已创建默认管理员账号: admin");
    Ok(())
}
