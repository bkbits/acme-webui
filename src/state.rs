use crate::auth::SessionManager;
use sea_orm::DatabaseConnection;
use tokio::sync::Semaphore;

pub struct AppState {
    pub db: DatabaseConnection,
    /// sa-token 风格会话管理（satoken-rust 未发布 crates.io，此处为等价实现）
    pub sessions: SessionManager,
    /// 同时执行颁发任务的全局并发上限
    pub issue_sem: Semaphore,
}

impl AppState {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            sessions: SessionManager::new(),
            issue_sem: Semaphore::new(2),
        }
    }
}

pub type SharedState = std::sync::Arc<AppState>;
