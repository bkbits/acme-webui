use crate::auth::{verify_password, AuthUser, TOKEN_HEADER};
use crate::entities::user;
use crate::error::{ok, AppError};
use crate::state::SharedState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let username = req.username.trim().to_string();
    if username.is_empty() || req.password.is_empty() {
        return Err(AppError::bad_request("用户名或密码不能为空"));
    }
    let found = user::Entity::find()
        .filter(user::Column::Username.eq(&username))
        .one(&state.db)
        .await?;
    let valid = found
        .as_ref()
        .map(|u| verify_password(&req.password, &u.password_hash))
        .unwrap_or(false);
    if !valid {
        return Err(AppError::bad_request("用户名或密码错误"));
    }
    let u = found.unwrap();
    let token = Uuid::new_v4().to_string();
    state.sessions.insert(&token, &u.username);
    Ok(ok(json!({ "token": token, "username": u.username })))
}

pub async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
    _user: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Some(v) = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok()) {
        state.sessions.remove(v.trim());
    }
    Ok(ok(json!({})))
}

pub async fn me(user: AuthUser) -> Result<Json<serde_json::Value>, AppError> {
    Ok(ok(json!({ "username": user.username })))
}

#[derive(Deserialize)]
pub struct ChangePwdReq {
    pub old_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<SharedState>,
    user: AuthUser,
    Json(req): Json<ChangePwdReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.new_password.len() < 6 {
        return Err(AppError::bad_request("新密码长度至少 6 位"));
    }
    let found = user::Entity::find()
        .filter(user::Column::Username.eq(&user.username))
        .one(&state.db)
        .await?;
    let u = found.ok_or_else(AppError::unauthorized)?;
    if !verify_password(&req.old_password, &u.password_hash) {
        return Err(AppError::bad_request("原密码错误"));
    }
    let mut am = user::ActiveModel {
        id: sea_orm::ActiveValue::Set(u.id),
        ..Default::default()
    };
    am.password_hash = sea_orm::ActiveValue::Set(crate::auth::hash_password(&req.new_password));
    am.update(&state.db).await?;
    Ok(ok(json!({ "changed": true })))
}
