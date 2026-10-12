mod api;
mod auth;
mod db;
mod entities;
mod error;
mod registry;
mod services;
mod state;
mod static_files;
mod util;

use clap::{ArgAction, CommandFactory, Parser};
use state::AppState;
use std::sync::Arc;

/// acme 证书自动申请续期服务平台
#[derive(Parser, Debug)]
#[command(
    name = "acme-webui",
    version,
    disable_help_flag = true,
    about = "acme 证书自动申请续期服务平台"
)]
struct Args {
    /// 监听地址
    #[arg(short = 'h', long = "host", default_value = "0.0.0.0")]
    host: String,

    /// 监听端口
    #[arg(short = 'p', long = "port", default_value_t = 6980)]
    port: u16,

    /// 打印帮助
    #[arg(long, action = ArgAction::SetTrue)]
    help: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.help {
        println!("{}", Args::command().render_help());
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // 数据库
    let db_path =
        std::env::var("ACME_WEBUI_DB").unwrap_or_else(|_| "data/acme-webui.db".to_string());
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let db = db::init(&db_path).await?;
    db::reset_issuing(&db).await?;
    db::seed_admin(&db).await?;

    let state: Arc<AppState> = Arc::new(AppState::new(db));

    // 周期调度器
    tokio::spawn(services::scheduler::run(state.clone()));

    let app = api::build_router(state);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let acme = services::acme::find_acme_sh();
    tracing::info!("====================================================");
    tracing::info!(" ACME WebUI 已启动");
    tracing::info!(" 监听地址: http://{addr}");
    tracing::info!(" 数据库: {db_path}");
    match &acme {
        Some(p) => {
            #[cfg(windows)]
            tracing::info!(" acme.sh: {} (WSL 内，已检测到，经 wsl 调用)", p.display());
            #[cfg(not(windows))]
            tracing::info!(" acme.sh: {} (已检测到)", p.display());
        }
        None => {
            let hint = services::acme::install_hint();
            tracing::warn!(
                " acme.sh: 未检测到，请先在 {} 上安装: {}",
                hint["os_name"],
                hint["primary_cmd"]
            );
        }
    }
    tracing::info!(" 默认账号: admin / admin123 (首次启动自动创建，请及时修改)");
    tracing::info!("====================================================");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("收到退出信号，服务已停止");
        })
        .await?;
    Ok(())
}
