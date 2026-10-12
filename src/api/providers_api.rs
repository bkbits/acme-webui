use crate::auth::AuthUser;
use crate::entities::{cert, provider};
use crate::error::{ok, AppError};
use crate::registry::{find_provider_type, PROVIDER_TYPES};
use crate::state::SharedState;
use crate::util;
use axum::extract::{Path, State};
use axum::Json;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub async fn schema(_user: AuthUser) -> Result<Json<serde_json::Value>, AppError> {
    Ok(ok(PROVIDER_TYPES))
}

#[derive(Deserialize)]
pub struct ProviderPayload {
    pub provider_type: String,
    pub credentials: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn env_key_first(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}
fn env_key_rest(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// 合法环境变量名: 字母/下划线开头，后续字母/数字/下划线
fn valid_env_key(key: &str) -> bool {
    let b = key.as_bytes();
    !b.is_empty() && env_key_first(b[0]) && b[1..].iter().all(|c| env_key_rest(*c))
}

/// 校验并归一化: 返回 (配置名称=运营商中文名, 凭据JSON)
/// 配置名称由 provider_type 决定，不由用户自定义；允许 schema 外的额外环境变量（需为合法环境变量名）
fn validate_payload(p: &ProviderPayload) -> Result<(String, String), AppError> {
    let t = find_provider_type(&p.provider_type)
        .ok_or_else(|| AppError::bad_request("不支持的 DNS 运营商"))?;
    let mut creds: HashMap<String, String> = HashMap::new();
    for (k, v) in &p.credentials {
        let key = k.trim();
        let v = v.trim();
        if key.is_empty() || v.is_empty() {
            continue;
        }
        if !valid_env_key(key) {
            return Err(AppError::bad_request(format!("非法的环境变量名: {key}")));
        }
        creds.insert(key.to_string(), v.to_string());
    }
    for f in t.fields.iter().filter(|f| f.required) {
        if !creds.contains_key(f.key) {
            return Err(AppError::bad_request(format!(
                "[{}] {} 不能为空",
                t.label, f.label
            )));
        }
    }
    if !t.require_any.is_empty() {
        let satisfied = t
            .require_any
            .iter()
            .any(|g| g.iter().all(|k| creds.contains_key(*k)));
        if !satisfied {
            let need = t
                .require_any
                .iter()
                .map(|g| g.join(" + "))
                .collect::<Vec<_>>()
                .join(" 或 ");
            return Err(AppError::bad_request(format!("凭据不完整，需提供: {need}")));
        }
    }
    if !t.no_creds && creds.is_empty() {
        return Err(AppError::bad_request(format!(
            "请为「{}」至少填写一项凭据（或添加其他环境变量）",
            t.label
        )));
    }
    Ok((
        t.label.to_string(),
        serde_json::to_string(&creds).unwrap_or_else(|_| "{}".into()),
    ))
}

fn provider_json(p: &provider::Model, cert_count: u64) -> serde_json::Value {
    let t = find_provider_type(&p.provider_type);
    json!({
        "id": p.id,
        "name": t.map(|t| t.label).unwrap_or(&p.name),
        "provider_type": p.provider_type,
        "type_label": t.map(|t| t.label).unwrap_or(p.provider_type.as_str()),
        "dns_hook": t.map(|t| t.dns_hook),
        "credentials": serde_json::from_str::<serde_json::Value>(&p.credentials).unwrap_or(json!({})),
        "enabled": p.enabled != 0,
        "cert_count": cert_count,
        "created_at": p.created_at,
        "updated_at": p.updated_at,
    })
}

pub async fn list(
    State(state): State<SharedState>,
    _user: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    let providers = provider::Entity::find()
        .order_by_asc(provider::Column::Id)
        .all(&state.db)
        .await?;
    let mut out = Vec::new();
    for p in &providers {
        let cert_count = cert::Entity::find()
            .filter(cert::Column::ProviderId.eq(p.id))
            .count(&state.db)
            .await?;
        out.push(provider_json(p, cert_count));
    }
    Ok(ok(out))
}

/// 新增或更新（同一运营商只允许一份配置，按 provider_type 幂等）
pub async fn create(
    State(state): State<SharedState>,
    _user: AuthUser,
    Json(payload): Json<ProviderPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (name, creds) = validate_payload(&payload)?;
    let now = util::now_str();
    let existing = provider::Entity::find()
        .filter(provider::Column::ProviderType.eq(&payload.provider_type))
        .one(&state.db)
        .await?;
    match existing {
        Some(m) => {
            let mut am = provider::ActiveModel {
                id: Set(m.id),
                ..Default::default()
            };
            am.name = Set(name);
            am.credentials = Set(creds);
            am.enabled = Set(if payload.enabled { 1 } else { 0 });
            am.updated_at = Set(now);
            am.update(&state.db).await?;
            Ok(ok(json!({ "id": m.id })))
        }
        None => {
            let am = provider::ActiveModel {
                name: Set(name),
                provider_type: Set(payload.provider_type),
                credentials: Set(creds),
                enabled: Set(if payload.enabled { 1 } else { 0 }),
                created_at: Set(now.clone()),
                updated_at: Set(now),
                ..Default::default()
            };
            let m = am.insert(&state.db).await?;
            Ok(ok(json!({ "id": m.id })))
        }
    }
}

pub async fn update(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
    Json(payload): Json<ProviderPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = provider::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("运营商配置不存在"))?;
    let (name, creds) = validate_payload(&payload)?;
    // 不允许切换为已存在配置的运营商类型
    if m.provider_type != payload.provider_type {
        let occupied = provider::Entity::find()
            .filter(provider::Column::ProviderType.eq(&payload.provider_type))
            .one(&state.db)
            .await?;
        if occupied.is_some() {
            return Err(AppError::conflict("该运营商已有配置，请前往对应菜单项编辑"));
        }
    }
    let mut am = provider::ActiveModel {
        id: Set(m.id),
        ..Default::default()
    };
    am.name = Set(name);
    am.provider_type = Set(payload.provider_type);
    am.credentials = Set(creds);
    am.enabled = Set(if payload.enabled { 1 } else { 0 });
    am.updated_at = Set(util::now_str());
    am.update(&state.db).await?;
    Ok(ok(json!({ "id": id })))
}

pub async fn remove(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = provider::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("运营商配置不存在"))?;
    let in_use = cert::Entity::find()
        .filter(cert::Column::ProviderId.eq(m.id))
        .count(&state.db)
        .await?;
    if in_use > 0 {
        return Err(AppError::conflict(format!(
            "该配置正被 {in_use} 张证书使用，请先删除或修改相关证书"
        )));
    }
    provider::Entity::delete_by_id(m.id).exec(&state.db).await?;
    Ok(ok(json!({ "deleted": true })))
}
