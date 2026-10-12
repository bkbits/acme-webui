pub mod auth_api;
pub mod certs_api;
pub mod meta_api;
pub mod providers_api;
pub mod records_api;

use crate::state::SharedState;
use crate::static_files;
use axum::routing::{get, post, put};
use axum::Router;

pub fn build_router(state: SharedState) -> Router {
    Router::new()
        // 认证
        .route("/api/auth/login", post(auth_api::login))
        .route("/api/auth/logout", post(auth_api::logout))
        .route("/api/auth/me", get(auth_api::me))
        .route("/api/auth/password", put(auth_api::change_password))
        // 元信息
        .route("/api/meta", get(meta_api::meta))
        // 运营商配置
        .route("/api/providers/schema", get(providers_api::schema))
        .route(
            "/api/providers",
            get(providers_api::list).post(providers_api::create),
        )
        .route(
            "/api/providers/{id}",
            put(providers_api::update).delete(providers_api::remove),
        )
        // 证书配置
        .route("/api/certs", get(certs_api::list).post(certs_api::create))
        .route("/api/certs/batch-delete", post(certs_api::batch_delete))
        .route(
            "/api/certs/{id}",
            put(certs_api::update).delete(certs_api::remove),
        )
        .route("/api/certs/{id}/renew", post(certs_api::renew))
        // 颁发记录
        .route("/api/records", get(records_api::list))
        .route("/api/records/batch-delete", post(records_api::batch_delete))
        .route(
            "/api/records/{id}",
            get(records_api::detail).delete(records_api::remove),
        )
        .with_state(state)
        // 前端静态资源（SPA）
        .fallback(static_files::static_handler)
}
