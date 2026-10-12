use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// 统一业务错误: HTTP 状态码与响应体 code 保持一致
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub msg: String,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, msg: msg.into() }
    }
    pub fn unauthorized() -> Self {
        Self { status: StatusCode::UNAUTHORIZED, msg: "未登录或登录已过期".into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, msg: msg.into() }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::CONFLICT, msg: msg.into() }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, msg: msg.into() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let code = self.status.as_u16();
        (self.status, Json(json!({ "code": code, "msg": self.msg }))).into_response()
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self {
        tracing::error!("数据库错误: {e}");
        AppError::internal(format!("数据库错误: {e}"))
    }
}

/// 统一成功响应: { "code": 0, "msg": "ok", "data": ... }
pub fn ok<T: serde::Serialize>(data: T) -> Json<serde_json::Value> {
    Json(json!({ "code": 0, "msg": "ok", "data": data }))
}
