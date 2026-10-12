use crate::entities::cert;
use crate::services::acme;
use crate::state::AppState;
use crate::util;
use chrono::{Duration, Local};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// 周期调度器: 定时扫描到期证书并触发自动续期
pub async fn run(state: std::sync::Arc<AppState>) {
    let interval: u64 = std::env::var("ACME_WEBUI_SCAN_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s| *s >= 1)
        .unwrap_or(60);
    tracing::info!("周期调度器已启动，扫描间隔 {interval} 秒");
    loop {
        if let Err(e) = tick(&state).await {
            tracing::error!("调度扫描出错: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
    }
}

async fn tick(state: &std::sync::Arc<AppState>) -> Result<(), sea_orm::DbErr> {
    let now = Local::now().naive_local();
    let certs = cert::Entity::find()
        .filter(cert::Column::Status.ne("issuing"))
        .all(&state.db)
        .await?;
    for c in certs {
        if c.status != "pending" && c.status != "issued" && c.status != "failed" {
            continue;
        }
        // 到期判定: 距上次颁发（无颁发记录则距创建时间）满 renewal_days 天
        let Some(base) =
            util::parse_dt(c.last_issued_at.as_deref().unwrap_or(&c.created_at))
        else {
            continue;
        };
        if now < base + Duration::days(c.renewal_days as i64) {
            continue;
        }
        // 失败退避: 30min * 2^连续失败次数，上限 24h
        if let Some(last_att) = c.last_attempt_at.as_deref().and_then(util::parse_dt) {
            let backoff = (1800i64 * (1i64 << c.fail_count.min(6))).min(86400);
            if now < last_att + Duration::seconds(backoff) {
                continue;
            }
        }
        // CAS 标记 issuing，与手动触发互斥
        let res = cert::Entity::update_many()
            .col_expr(cert::Column::Status, Expr::value("issuing"))
            .filter(cert::Column::Id.eq(c.id))
            .filter(cert::Column::Status.ne("issuing"))
            .exec(&state.db)
            .await?;
        if res.rows_affected == 1 {
            tracing::info!(
                "证书 #{} [{}] 到达续期时间，自动触发颁发",
                c.id,
                if c.name.is_empty() { c.domains.as_str() } else { c.name.as_str() }
            );
            tokio::spawn(acme::issue_cert(state.clone(), c.id, "auto"));
        }
    }
    Ok(())
}
