use crate::auth::AuthUser;
use crate::entities::{cert, provider};
use crate::error::{ok, AppError};
use crate::registry::{ca_valid, CA_DEFAULT};
use crate::state::SharedState;
use crate::util;
use axum::extract::{Path, State};
use axum::Json;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;

static DOMAIN_RE: OnceLock<regex::Regex> = OnceLock::new();

fn domain_re() -> &'static regex::Regex {
    DOMAIN_RE.get_or_init(|| {
        regex::Regex::new(r"(?i)^(\*\.)?([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$").unwrap()
    })
}

#[derive(Deserialize)]
pub struct CertPayload {
    pub name: Option<String>,
    pub provider_id: i32,
    pub domains: Vec<String>,
    pub cert_path: String,
    pub key_path: String,
    pub ca: Option<String>,
    pub renewal_days: i32,
    pub post_command: Option<String>,
}

struct Validated {
    name: String,
    domains: Vec<String>,
    cert_path: String,
    key_path: String,
    ca: String,
    post_command: String,
}

fn validate(p: &CertPayload) -> Result<Validated, AppError> {
    let name = p.name.clone().unwrap_or_default().trim().to_string();
    let mut domains: Vec<String> = Vec::new();
    if p.domains.is_empty() {
        return Err(AppError::bad_request("至少填写一个域名"));
    }
    for d in &p.domains {
        let d = d.trim().to_lowercase();
        if d.is_empty() {
            continue;
        }
        if !domain_re().is_match(&d) {
            return Err(AppError::bad_request(format!("域名格式不正确: {d}")));
        }
        if domains.contains(&d) {
            return Err(AppError::bad_request(format!("域名重复: {d}")));
        }
        domains.push(d);
    }
    if domains.is_empty() {
        return Err(AppError::bad_request("至少填写一个域名"));
    }
    let cert_path = p.cert_path.trim().to_string();
    let key_path = p.key_path.trim().to_string();
    if cert_path.is_empty() || key_path.is_empty() {
        return Err(AppError::bad_request("公钥 / 私钥位置不能为空"));
    }
    if !std::path::Path::new(&cert_path).is_absolute()
        || !std::path::Path::new(&key_path).is_absolute()
    {
        return Err(AppError::bad_request("公钥 / 私钥位置必须为绝对路径"));
    }
    if std::path::Path::new(&cert_path).file_name().is_none()
        || std::path::Path::new(&key_path).file_name().is_none()
    {
        return Err(AppError::bad_request(
            "公钥 / 私钥位置必须包含文件名，不能以分隔符结尾",
        ));
    }
    if cert_path == key_path {
        return Err(AppError::bad_request("公钥与私钥位置不能相同"));
    }
    #[cfg(windows)]
    if cert_path.to_lowercase() == key_path.to_lowercase() {
        return Err(AppError::bad_request("公钥与私钥位置不能相同"));
    }
    if !(1..=3650).contains(&p.renewal_days) {
        return Err(AppError::bad_request("续期周期需在 1 - 3650 天之间"));
    }
    let ca =
        p.ca.clone()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| CA_DEFAULT.to_string());
    if !ca_valid(&ca) {
        return Err(AppError::bad_request("不支持的 CA 机构"));
    }
    let post_command = p
        .post_command
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(Validated {
        name,
        domains,
        cert_path,
        key_path,
        ca,
        post_command,
    })
}

fn cert_json(c: &cert::Model, p: Option<&provider::Model>) -> serde_json::Value {
    let domains: Vec<String> = serde_json::from_str(&c.domains).unwrap_or_default();
    let next = util::add_days(
        c.last_issued_at.as_deref().unwrap_or(&c.created_at),
        c.renewal_days as i64,
    )
    .map(util::fmt_naive);
    json!({
        "id": c.id,
        "name": c.name,
        "provider_id": c.provider_id,
        "provider_type": p.map(|p| p.provider_type.clone()),
        "provider_name": p.map(|p| p.name.clone()),
        "domains": domains,
        "cert_path": c.cert_path,
        "key_path": c.key_path,
        "ca": c.ca,
        "renewal_days": c.renewal_days,
        "post_command": c.post_command,
        "status": c.status,
        "last_issued_at": c.last_issued_at,
        "cert_expire_at": c.cert_expire_at,
        "next_renew_at": next,
        "last_error": c.last_error,
        "created_at": c.created_at,
        "updated_at": c.updated_at,
    })
}

pub async fn list(
    State(state): State<SharedState>,
    _user: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    let certs = cert::Entity::find()
        .order_by_asc(cert::Column::Id)
        .all(&state.db)
        .await?;
    let providers: HashMap<i32, provider::Model> = provider::Entity::find()
        .all(&state.db)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let list: Vec<serde_json::Value> = certs
        .iter()
        .map(|c| cert_json(c, providers.get(&c.provider_id)))
        .collect();
    Ok(ok(list))
}

pub async fn create(
    State(state): State<SharedState>,
    _user: AuthUser,
    Json(payload): Json<CertPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let v = validate(&payload)?;
    // 运营商配置必须存在
    provider::Entity::find_by_id(payload.provider_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::bad_request("所选 DNS 运营商配置不存在"))?;
    let now = util::now_str();
    let am = cert::ActiveModel {
        name: Set(v.name),
        provider_id: Set(payload.provider_id),
        domains: Set(serde_json::to_string(&v.domains).unwrap_or_else(|_| "[]".into())),
        cert_path: Set(v.cert_path),
        key_path: Set(v.key_path),
        ca: Set(v.ca),
        renewal_days: Set(payload.renewal_days),
        post_command: Set(v.post_command),
        status: Set("pending".to_string()),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    };
    let m = am.insert(&state.db).await?;
    Ok(ok(json!({ "id": m.id })))
}

pub async fn update(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
    Json(payload): Json<CertPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = cert::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("证书配置不存在"))?;
    if m.status == "issuing" {
        return Err(AppError::conflict("该证书正在颁发中，请稍后再修改"));
    }
    let v = validate(&payload)?;
    provider::Entity::find_by_id(payload.provider_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::bad_request("所选 DNS 运营商配置不存在"))?;
    let mut am = cert::ActiveModel {
        id: Set(m.id),
        ..Default::default()
    };
    am.name = Set(v.name);
    am.provider_id = Set(payload.provider_id);
    am.domains = Set(serde_json::to_string(&v.domains).unwrap_or_else(|_| "[]".into()));
    am.cert_path = Set(v.cert_path);
    am.key_path = Set(v.key_path);
    am.ca = Set(v.ca);
    am.renewal_days = Set(payload.renewal_days);
    am.post_command = Set(v.post_command);
    am.updated_at = Set(util::now_str());
    am.update(&state.db).await?;
    Ok(ok(json!({ "id": id })))
}

pub async fn remove(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = cert::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("证书配置不存在"))?;
    if m.status == "issuing" {
        return Err(AppError::conflict(format!(
            "证书 [{}] 正在颁发中，不能删除",
            display_name(&m)
        )));
    }
    cert::Entity::delete_by_id(m.id).exec(&state.db).await?;
    Ok(ok(json!({ "deleted": true })))
}

#[derive(Deserialize)]
pub struct BatchDeleteReq {
    pub ids: Vec<i32>,
}

pub async fn batch_delete(
    State(state): State<SharedState>,
    _user: AuthUser,
    Json(req): Json<BatchDeleteReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.ids.is_empty() {
        return Err(AppError::bad_request("未选择要删除的证书"));
    }
    let certs = cert::Entity::find()
        .filter(cert::Column::Id.is_in(req.ids.clone()))
        .all(&state.db)
        .await?;
    if let Some(c) = certs.iter().find(|c| c.status == "issuing") {
        return Err(AppError::conflict(format!(
            "证书 [{}] 正在颁发中，请先等待完成",
            display_name(c)
        )));
    }
    let n = certs.len();
    cert::Entity::delete_many()
        .filter(cert::Column::Id.is_in(req.ids))
        .exec(&state.db)
        .await?;
    Ok(ok(json!({ "deleted": n })))
}

/// 立即续期: 标记 issuing 后立即返回，由后台任务执行颁发
pub async fn renew(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let c = cert::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("证书配置不存在"))?;
    if c.status == "issuing" {
        return Err(AppError::conflict("该证书正在颁发中"));
    }
    let prov = provider::Entity::find_by_id(c.provider_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::conflict("关联的 DNS 运营商配置不存在，无法颁发"))?;
    if prov.enabled == 0 {
        return Err(AppError::conflict("关联的 DNS 运营商配置已停用，请先启用"));
    }
    // CAS: 仅当当前状态不是 issuing 时标记成功
    let res = cert::Entity::update_many()
        .col_expr(cert::Column::Status, Expr::value("issuing"))
        .filter(cert::Column::Id.eq(id))
        .filter(cert::Column::Status.ne("issuing"))
        .exec(&state.db)
        .await?;
    if res.rows_affected == 0 {
        return Err(AppError::conflict("该证书正在颁发中"));
    }
    tracing::info!("手动触发颁发: 证书 #{}", id);
    tokio::spawn(crate::services::acme::issue_cert(
        state.clone(),
        id,
        "manual",
    ));
    Ok(ok(json!({ "started": true })))
}

fn display_name(c: &cert::Model) -> String {
    if !c.name.trim().is_empty() {
        c.name.trim().to_string()
    } else {
        serde_json::from_str::<Vec<String>>(&c.domains)
            .unwrap_or_default()
            .join(",")
    }
}
