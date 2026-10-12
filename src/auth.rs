use crate::error::AppError;
use crate::state::AppState;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// sa-token 风格的 token 请求头名称
pub const TOKEN_HEADER: &str = "satoken";
const TOKEN_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

pub struct Session {
    pub username: String,
    pub expires_at: Instant,
}

/// 内存会话管理器（进程重启后需重新登录），支持滑动续期
#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, token: &str, username: &str) {
        self.sessions.lock().unwrap().insert(
            token.to_string(),
            Session {
                username: username.to_string(),
                expires_at: Instant::now() + TOKEN_TTL,
            },
        );
    }

    /// 校验 token，未过期时滑动续期
    pub fn validate(&self, token: &str) -> Option<String> {
        let mut map = self.sessions.lock().unwrap();
        match map.get_mut(token) {
            Some(s) => {
                if s.expires_at < Instant::now() {
                    map.remove(token);
                    return None;
                }
                s.expires_at = Instant::now() + TOKEN_TTL;
                Some(s.username.clone())
            }
            None => None,
        }
    }

    pub fn remove(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }
}

pub fn hash_password(password: &str) -> String {
    // 两个 UUIDv4 拼出 32 字节随机盐，避免引入额外 rand 依赖
    let mut salt_bytes = [0u8; 32];
    salt_bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    salt_bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    let salt = SaltString::encode_b64(&salt_bytes).expect("盐值编码失败");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("密码哈希计算失败")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// 已登录用户提取器: 无有效 token 时返回 401
pub struct AuthUser {
    pub username: String,
}

impl FromRequestParts<std::sync::Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &std::sync::Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match extract_token(parts).and_then(|t| state.sessions.validate(&t)) {
            Some(username) => Ok(AuthUser { username }),
            None => Err(AppError::unauthorized()),
        }
    }
}

pub fn extract_token(parts: &Parts) -> Option<String> {
    if let Some(v) = parts.headers.get(TOKEN_HEADER) {
        if let Ok(s) = v.to_str() {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    if let Some(v) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = v.to_str() {
            if let Some(t) = s.strip_prefix("Bearer ") {
                if !t.trim().is_empty() {
                    return Some(t.trim().to_string());
                }
            }
        }
    }
    None
}
