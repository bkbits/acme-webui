use crate::auth::AuthUser;
use crate::entities::record;
use crate::error::{ok, AppError};
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::Json;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub cert_id: Option<i32>,
    pub success: Option<bool>,
}

fn record_json(r: &record::Model, with_output: bool) -> serde_json::Value {
    let domains: Vec<String> = serde_json::from_str(&r.domains).unwrap_or_default();
    let label = crate::registry::find_provider_type(&r.provider_type)
        .map(|t| t.label)
        .unwrap_or(r.provider_type.as_str());
    let mut v = json!({
        "id": r.id,
        "cert_id": r.cert_id,
        "cert_name": r.cert_name,
        "trigger": r.trigger_type,
        "issued_at": r.issued_at,
        "expire_at": r.expire_at,
        "provider_type": r.provider_type,
        "provider_label": label,
        "provider_name": r.provider_name,
        "domains": domains,
        "cert_path": r.cert_path,
        "key_path": r.key_path,
        "ca": r.ca,
        "success": r.success == 1,
        "message": r.message,
        "has_output": !r.command_output.is_empty(),
    });
    if with_output {
        v["command_output"] = json!(r.command_output);
    }
    v
}

pub async fn list(
    State(state): State<SharedState>,
    _user: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let mut sel = record::Entity::find().order_by_desc(record::Column::Id);
    if let Some(cid) = q.cert_id {
        sel = sel.filter(record::Column::CertId.eq(cid));
    }
    if let Some(s) = q.success {
        sel = sel.filter(record::Column::Success.eq(if s { 1 } else { 0 }));
    }
    let total = sel
        .clone()
        .paginate(&state.db, page_size as u64)
        .num_items()
        .await?;
    let items = sel
        .paginate(&state.db, page_size as u64)
        .fetch_page(page - 1)
        .await?;
    let list: Vec<serde_json::Value> = items.iter().map(|r| record_json(r, false)).collect();
    Ok(ok(json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "list": list,
    })))
}

pub async fn detail(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let r = record::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("颁发记录不存在"))?;
    Ok(ok(record_json(&r, true)))
}

pub async fn remove(
    State(state): State<SharedState>,
    _user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let n = record::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?
        .rows_affected;
    if n == 0 {
        return Err(AppError::not_found("颁发记录不存在"));
    }
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
        return Err(AppError::bad_request("未选择要删除的颁发记录"));
    }
    let n = record::Entity::delete_many()
        .filter(record::Column::Id.is_in(req.ids))
        .exec(&state.db)
        .await?
        .rows_affected;
    Ok(ok(json!({ "deleted": n })))
}
