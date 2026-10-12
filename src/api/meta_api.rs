use crate::auth::AuthUser;
use crate::error::{ok, AppError};
use crate::services::acme::find_acme_sh;
use axum::Json;
use serde_json::json;

/// 元信息: CA 列表 / acme.sh 检测 + 安装指引
pub async fn meta(_user: AuthUser) -> Result<Json<serde_json::Value>, AppError> {
    let acme = find_acme_sh();
    Ok(ok(json!({
        "cas": crate::registry::CA_LIST.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
        "ca_default": crate::registry::CA_DEFAULT,
        "acme_sh": {
            "found": acme.is_some(),
            "path": acme.map(|p| p.display().to_string()),
            "install": crate::services::acme::install_hint(),
        },
    })))
}
