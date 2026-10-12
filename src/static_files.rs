use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// 前端构建产物（web/dist），编译时嵌入；debug 模式下从磁盘实时读取
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        // text/* 统一声明 charset=utf-8，否则浏览器按系统默认编码（如 GBK）解码产生乱码
        let content_type = if mime.type_() == mime_guess::mime::TEXT {
            format!("{mime}; charset=utf-8")
        } else {
            mime.as_ref().to_string()
        };
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, cache)
            .body(Body::from(file.data.into_owned()))
            .unwrap();
    }

    // SPA 回退到 index.html
    match Assets::get("index.html") {
        Some(file) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(file.data.into_owned()))
            .unwrap(),
        None => (
            StatusCode::NOT_FOUND,
            "前端资源未构建: 请先在 web/ 目录执行 bun install && bun run build，再重新编译后端",
        )
            .into_response(),
    }
}
