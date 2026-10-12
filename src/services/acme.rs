use crate::entities::{cert, provider, record};
use crate::registry::{ca_server_key, find_provider_type};
use crate::state::AppState;
use crate::util;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const ISSUE_TIMEOUT: Duration = Duration::from_secs(600);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(120);
const POST_CMD_TIMEOUT: Duration = Duration::from_secs(300);

/// acme.sh 官方安装命令（email 需替换为实际邮箱）
pub const ACME_INSTALL_CMD: &str = "curl https://get.acme.sh | sh -s email=you@example.com";

/// 按当前运行平台生成 acme.sh 安装指引（meta 接口返回，前端展示）
pub fn install_hint() -> serde_json::Value {
    use serde_json::json;
    let os = std::env::consts::OS; // linux / macos / windows
    let os_name = match os {
        "windows" => "Windows",
        "macos" => "macOS",
        _ => "Linux",
    };
    let (steps, notes): (Vec<serde_json::Value>, Vec<String>) = if os == "windows" {
        (
            vec![
                json!({
                    "title": "1. 安装 WSL Ubuntu（PowerShell 管理员执行）",
                    "commands": ["wsl --install"],
                }),
                json!({
                    "title": "2. 在 WSL（Ubuntu）内安装 acme.sh",
                    "commands": [ACME_INSTALL_CMD],
                }),
            ],
            vec![
                "acme.sh 是 Shell 脚本，Windows 无法直接调用，服务通过 wsl 命令自动调用 WSL 内的 acme.sh".into(),
                "查找顺序: ACME_WEBUI_ACME_SH 环境变量 > wsl which ~/.acme.sh/acme.sh > wsl which acme.sh（ACME_WEBUI_WSL_DISTRO 可指定发行版，默认发行版）".into(),
                "证书公钥/私钥位置配置为 Windows 绝对路径，如 C:/certs/example.com/fullchain.pem；颁发成功后证书从 WSL 临时目录 ~/.certs/ 移动到该位置（经 /mnt/<盘符> 映射），不会留在 WSL 内".into(),
            ],
        )
    } else {
        (
            vec![json!({
                "title": "安装 acme.sh",
                "commands": [ACME_INSTALL_CMD],
            })],
            vec![],
        )
    };
    let mut notes = notes;
    notes.push("将命令中的 email=you@example.com 替换为你的实际邮箱（作为证书联系人）".into());
    notes.push("安装后服务会自动检测到 acme.sh，无需重启".into());
    json!({
        "os": os,
        "os_name": os_name,
        "primary_cmd": ACME_INSTALL_CMD,
        "steps": steps,
        "notes": notes,
    })
}

/// 查找 acme.sh。
/// Unix: ACME_WEBUI_ACME_SH > ~/.acme.sh/acme.sh > PATH
/// Windows: acme.sh 运行于 WSL 内，按顺序: ACME_WEBUI_ACME_SH（WSL 内部路径 / wsl$ 形式 / C:\ 盘符路径，
/// 均转为 WSL 内部路径后验证，wsl$ 形式经 wsl_override_to_internal 归一化）
/// > wsl which ~/.acme.sh/acme.sh > wsl which acme.sh（检测结果缓存）
pub fn find_acme_sh() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // 1) 显式环境变量（统一转 WSL 内部路径并验证存在）
        if let Ok(raw) = std::env::var("ACME_WEBUI_ACME_SH") {
            let raw = raw.trim().to_string();
            if !raw.is_empty() {
                let internal = if let Some(i) = wsl_override_to_internal(&raw) {
                    i
                } else if raw.starts_with('/') {
                    // POSIX 风格绝对路径: 直接作为 WSL 内部路径
                    raw.clone()
                } else {
                    // Windows 盘符路径（C:\... / C:/...）: 转 /mnt 映射
                    to_wsl_path(&raw)
                };
                if wsl_file_exists(&internal) {
                    return Some(PathBuf::from(internal));
                }
            }
        }
        // 2) WSL 内自动检测（结果缓存，避免重复冷启动发行版）
        static CACHED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        CACHED.get_or_init(wsl_detect_default).clone()
    }
    #[cfg(not(windows))]
    {
        if let Ok(p) = std::env::var("ACME_WEBUI_ACME_SH") {
            let p = PathBuf::from(p.trim());
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home).join(".acme.sh").join("acme.sh");
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                let p = dir.join("acme.sh");
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// 读取环境变量指定的 WSL 发行版（None 表示默认发行版）
#[cfg(windows)]
fn wsl_distro() -> Option<String> {
    std::env::var("ACME_WEBUI_WSL_DISTRO")
        .ok()
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
}

/// 通过 WSL 的 bash -c 执行命令（wsl 不存在或启动失败返回 None）
#[cfg(windows)]
fn wsl_bash(cmdline: &str) -> Option<std::process::Output> {
    let mut c = std::process::Command::new("wsl");
    if let Some(d) = wsl_distro() {
        c.arg("-d").arg(d);
    }
    c.args(["-e", "bash", "-c", cmdline])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
}

/// 将指向 WSL 的 Windows 路径（wsl$ 或 wsl.localhost 前缀）转为 WSL 内部路径
#[cfg(windows)]
fn wsl_override_to_internal(p: &str) -> Option<String> {
    let t = p.trim_start_matches(['\\', '/']);
    let rest = t
        .strip_prefix("wsl$")
        .or_else(|| t.strip_prefix("wsl.localhost"))?;
    let rest = rest.trim_start_matches(['\\', '/']);
    let after_distro = rest
        .splitn(2, ['\\', '/'])
        .nth(1)?
        .trim_start_matches(['\\', '/']);
    if after_distro.is_empty() {
        return None;
    }
    Some(format!("/{}", after_distro.replace('\\', "/")))
}

/// 判断 WSL 内部路径（文件）是否存在
#[cfg(windows)]
fn wsl_file_exists(internal: &str) -> bool {
    match wsl_bash(&format!("test -f '{}'", sh_single_quote(internal))) {
        Some(out) => out.status.success(),
        None => false,
    }
}

/// 自动检测 WSL 发行版内的 acme.sh: which ~/.acme.sh/acme.sh > which acme.sh
#[cfg(windows)]
fn wsl_detect_default() -> Option<PathBuf> {
    for cmd in ["which ~/.acme.sh/acme.sh", "which acme.sh"] {
        if let Some(out) = wsl_bash(cmd) {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if out.status.success() && s.starts_with('/') {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

/// 获取 WSL 当前用户主目录（构造 ~/.certs 临时目录用）
#[cfg(windows)]
fn wsl_home() -> Option<String> {
    let out = wsl_bash("printf '%s' \"$HOME\"")?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && s.starts_with('/') {
        Some(s)
    } else {
        None
    }
}

/// 单引号包裹并转义内部单引号（拼接 WSL bash 命令行用）
#[cfg(windows)]
fn sh_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'"))
}

fn base_shell() -> Command {
    #[cfg(unix)]
    let c = Command::new("sh");
    #[cfg(windows)]
    let mut c = Command::new("cmd");
    #[cfg(windows)]
    c.arg("/C");
    c
}

/// 构建 acme.sh 调用命令。
/// Unix: sh <script> <args>，凭据经环境变量传入
/// Windows: 拼接 bash -c 命令行 "<凭据内联> '<script>' <args>"（经 wsl [-d 发行版] -e 执行）
#[cfg(unix)]
fn build_acme_command(acme: &PathBuf, args: &[String], envs: &HashMap<String, String>) -> Command {
    let mut c = Command::new("sh");
    c.arg(acme.as_os_str());
    c.args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    c
}

#[cfg(windows)]
fn build_acme_command(acme: &PathBuf, args: &[String], envs: &HashMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(envs.len() + args.len() + 1);
    for (k, v) in envs {
        parts.push(format!("{}={}", k, sh_single_quote(v)));
    }
    parts.push(sh_single_quote(acme.to_string_lossy().as_ref()));
    for a in args {
        parts.push(sh_single_quote(a));
    }
    parts.join(" ")
}

/// 证书路径转 WSL 内可访问路径：C:\a\b -> /mnt/c/a/b（相对路径按工作目录绝对化）；
/// 非 Windows 平台原样返回
#[cfg(windows)]
fn to_wsl_path(p: &str) -> String {
    let path = std::path::Path::new(p);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };
    let s = abs.to_string_lossy().into_owned();
    let b = s.as_bytes();
    if b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {
        let drive = b[0].to_ascii_lowercase() as char;
        format!("/mnt/{}/{}", drive, s[3..].replace('\\', "/"))
    } else {
        s.replace('\\', "/")
    }
}

#[cfg(not(windows))]
fn to_wsl_path(p: &str) -> String {
    p.to_string()
}

/// Windows 路径的父目录转 WSL 路径（移动证书前 mkdir -p 用）
#[cfg(windows)]
fn parent_wsl(p: &str) -> String {
    let parent = std::path::Path::new(p)
        .parent()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string());
    to_wsl_path(&parent)
}

/// 取路径的文件名部分
#[cfg(windows)]
fn file_name_of(p: &str) -> Result<String, String> {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| format!("路径缺少文件名: {p}"))
}

/// 执行命令: 捕获 stdout/stderr，带超时（超时后 kill_on_drop 终止进程）
async fn run_cmd(mut cmd: Command, dur: Duration) -> Result<(bool, String), String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| format!("启动命令失败: {e}"))?;
    match timeout(dur, child.wait_with_output()).await {
        Err(_) => {
            // Windows: 杀掉 wsl.exe 后 WSL 内的 acme.sh 可能残留，尽力清理
            // （方括号写法避免 pkill 匹配到自身命令行）
            #[cfg(windows)]
            let _ = wsl_bash("pkill -f '[.]acme[.]sh/acme[.]sh'");
            Err("执行超时".to_string())
        }
        Ok(Err(e)) => Err(format!("执行失败: {e}")),
        Ok(Ok(out)) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&out.stdout));
            combined.push('\n');
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            Ok((out.status.success(), combined))
        }
    }
}

/// 在 WSL 内执行原始 bash 命令行（经 wsl [-d 发行版] -e bash -c，带超时）
#[cfg(windows)]
async fn wsl_run(cmdline: &str, dur: Duration) -> Result<(bool, String), String> {
    let mut c = Command::new("wsl");
    if let Some(d) = wsl_distro() {
        c.arg("-d").arg(d);
    }
    c.args(["-e", "bash", "-c"]).arg(cmdline);
    run_cmd(c, dur).await
}

/// 执行 acme.sh 脚本，带超时。
/// pre 为 bash 前置命令，&& 连接在 acme.sh 之前执行（Windows 下用于准备 WSL 临时目录）
async fn run_script(
    acme: &PathBuf,
    args: &[String],
    envs: &HashMap<String, String>,
    pre: &str,
    dur: Duration,
) -> Result<(bool, String), String> {
    #[cfg(unix)]
    {
        let mut cmd = build_acme_command(acme, args, envs);
        run_cmd(cmd, dur).await
    }
    #[cfg(windows)]
    {
        let mut cmdline = build_acme_command(acme, args, envs);
        if !pre.is_empty() {
            cmdline = format!("{} && {cmdline}", pre);
        }
        wsl_run(&cmdline, dur).await
    }
}

/// 解析证书文件中的到期时间（本地时区, YYYY-MM-DD HH:mm:ss）
fn parse_expire(pem: &[u8]) -> Result<String, String> {
    let (_, pem_obj) =
        x509_parser::pem::parse_x509_pem(pem).map_err(|e| format!("解析证书失败: {e}"))?;
    let crt = pem_obj
        .parse_x509()
        .map_err(|e| format!("解析证书内容失败: {e}"))?;
    let dt = crt.validity().not_after.to_datetime();
    use chrono::TimeZone;
    let local = chrono::Local
        .timestamp_opt(dt.unix_timestamp(), dt.nanosecond())
        .single()
        .ok_or("证书到期时间转换失败")?;
    Ok(local.format(crate::util::TIME_FMT).to_string())
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

/// 颁发单个证书（手动/自动共用），无论成败都写入颁发记录
pub async fn issue_cert(state: std::sync::Arc<AppState>, cert_id: i32, trigger: &'static str) {
    // 全局并发限制（排队期间状态已为 issuing）
    let _permit = state.issue_sem.acquire().await;
    let cert = match cert::Entity::find_by_id(cert_id).one(&state.db).await {
        Ok(Some(c)) => c,
        _ => {
            tracing::info!("证书 #{cert_id} 已被删除，跳过颁发");
            return;
        }
    };
    let disp = display_name(&cert);
    tracing::info!("trigger={} 开始颁发证书 #{} {}", trigger, cert_id, disp);

    let mut output = String::new();
    let provider = provider::Entity::find_by_id(cert.provider_id)
        .one(&state.db)
        .await
        .ok()
        .flatten();
    let mut post_err: Option<String> = None;

    let result: Result<Option<String>, String> = async {
        let prov = provider
            .as_ref()
            .ok_or("关联的 DNS 运营商配置不存在或已删除")?;
        if prov.enabled == 0 {
            return Err("关联的 DNS 运营商配置已停用".into());
        }
        let schema = find_provider_type(&prov.provider_type).ok_or("不支持的运营商类型")?;
        let domains: Vec<String> = serde_json::from_str(&cert.domains).unwrap_or_default();
        if domains.is_empty() {
            return Err("证书未配置域名".into());
        }
        let acme = find_acme_sh().ok_or_else(|| {
            let hint = install_hint();
            format!(
                "未找到 acme.sh，请先安装（运行环境: {}）: {}",
                hint["os_name"], hint["primary_cmd"]
            )
        })?;

        // 凭据 → 环境变量
        let mut envs: HashMap<String, String> = HashMap::new();
        let creds: HashMap<String, String> =
            serde_json::from_str(&prov.credentials).unwrap_or_default();
        for (k, v) in creds {
            envs.insert(k, v);
        }

        // 确保证书目录存在
        for p in [&cert.cert_path, &cert.key_path] {
            if let Some(parent) = std::path::Path::new(p).parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("创建目录失败 {}: {e}", parent.display()))?;
            }
        }

        // 1) 颁发（--force: 本平台自行控制周期）
        let mut args: Vec<String> = vec!["--issue".into(), "--dns".into(), schema.dns_hook.into()];
        for d in &domains {
            args.push("-d".into());
            args.push(d.clone());
        }
        args.push("--server".into());
        args.push(ca_server_key(&cert.ca).to_string());
        args.push("--keylength".into());
        args.push("ec-256".into());
        args.push("--force".into());

        // Windows: 同一次调用内直接写入 WSL 临时目录（~/.certs/<主域名>-<证书id>），
        // 成功后再移动到 Windows 配置位置，避免 acme.sh 直接写 /mnt 映射盘
        #[cfg(windows)]
        let (pre, tmp_dir, cert_base, key_base): (String, String, String, String) = {
            let home = wsl_home().ok_or("获取 WSL 用户主目录失败")?;
            let cert_base = file_name_of(&cert.cert_path)?;
            let key_base = file_name_of(&cert.key_path)?;
            let tmp_dir = format!("{}/.certs/{}-{}", home, domains[0], cert.id);
            let pre = format!(
                "rm -rf {} && mkdir -p {}",
                sh_single_quote(&tmp_dir),
                sh_single_quote(&tmp_dir)
            );
            args.push("--fullchain-file".into());
            args.push(format!("{tmp_dir}/{cert_base}"));
            args.push("--key-file".into());
            args.push(format!("{tmp_dir}/{key_base}"));
            (pre, tmp_dir, cert_base, key_base)
        };
        #[cfg(not(windows))]
        let pre = String::new();

        output.push_str(&format!("$ acme.sh {}\n", args.join(" ")));
        let (ok, out) = match run_script(&acme, &args, &envs, &pre, ISSUE_TIMEOUT).await {
            Ok(v) => v,
            Err(e) => {
                output.push_str(&format!("\n[error] {e}\n"));
                return Err(format!("acme.sh 颁发失败: {e}"));
            }
        };
        output.push_str(&out);
        output.push('\n');
        if !ok {
            return Err("acme.sh 颁发失败，详见命令执行结果".into());
        }

        // 2) Windows: 把 WSL 临时目录中的证书逐文件移动到 Windows 配置位置（/mnt/<盘符> 映射）
        #[cfg(windows)]
        {
            let mv_cmdline = format!(
                "mkdir -p {} {} && mv {} {} && mv {} {} && rm -rf {}",
                sh_single_quote(&parent_wsl(&cert.cert_path)),
                sh_single_quote(&parent_wsl(&cert.key_path)),
                sh_single_quote(&format!("{tmp_dir}/{cert_base}")),
                sh_single_quote(&to_wsl_path(&cert.cert_path)),
                sh_single_quote(&format!("{tmp_dir}/{key_base}")),
                sh_single_quote(&to_wsl_path(&cert.key_path)),
                sh_single_quote(&tmp_dir)
            );
            output.push_str(&format!("\n$ wsl -e bash -c {mv_cmdline}\n"));
            let (ok, out) = match wsl_run(&mv_cmdline, INSTALL_TIMEOUT).await {
                Ok(v) => v,
                Err(e) => {
                    output.push_str(&format!("\n[error] {e}\n"));
                    return Err(format!("移动证书文件失败: {e}"));
                }
            };
            output.push_str(&out);
            output.push('\n');
            if !ok {
                return Err("移动证书文件到 Windows 位置失败，详见命令执行结果".into());
            }
        }
        // 2) Unix: 安装证书到指定位置
        #[cfg(not(windows))]
        {
            let primary = domains[0].clone();
            let install_args: Vec<String> = vec![
                "--install-cert".into(),
                "-d".into(),
                primary,
                "--ecc".into(),
                "--fullchain-file".into(),
                to_wsl_path(&cert.cert_path),
                "--key-file".into(),
                to_wsl_path(&cert.key_path),
            ];
            output.push_str(&format!("\n$ acme.sh {}\n", install_args.join(" ")));
            let (ok, out) = match run_script(&acme, &install_args, &envs, "", INSTALL_TIMEOUT).await
            {
                Ok(v) => v,
                Err(e) => {
                    output.push_str(&format!("\n[error] {e}\n"));
                    return Err(format!("安装证书文件失败: {e}"));
                }
            };
            output.push_str(&out);
            output.push('\n');
            if !ok {
                return Err("acme.sh 安装证书文件失败，详见命令执行结果".into());
            }
        }

        // 私钥文件权限 600
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&cert.key_path, std::fs::Permissions::from_mode(0o600));
        }

        // 3) 解析到期时间
        let pem = tokio::fs::read(&cert.cert_path)
            .await
            .map_err(|e| format!("读取证书文件失败: {e}"))?;
        let expire = parse_expire(&pem)?;

        // 4) 完成后命令
        let post = cert.post_command.trim();
        if !post.is_empty() {
            output.push_str(&format!("\n$ {post}\n"));
            let mut c = base_shell();
            #[cfg(unix)]
            c.arg("-c").arg(post);
            #[cfg(windows)]
            c.arg(post);
            c.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .kill_on_drop(true);
            match c.spawn() {
                Ok(child) => match timeout(POST_CMD_TIMEOUT, child.wait_with_output()).await {
                    Ok(Ok(out)) => {
                        output.push_str(&String::from_utf8_lossy(&out.stdout));
                        output.push_str(&String::from_utf8_lossy(&out.stderr));
                        output.push('\n');
                        if !out.status.success() {
                            let msg =
                                format!("完成后命令执行失败（退出码 {:?}）", out.status.code());
                            output.push_str(&format!("[error] {msg}\n"));
                            post_err = Some(msg);
                        }
                    }
                    Ok(Err(e)) => {
                        let msg = format!("完成后命令执行失败: {e}");
                        output.push_str(&format!("[error] {msg}\n"));
                        post_err = Some(msg);
                    }
                    Err(_) => {
                        let msg = "完成后命令执行超时".to_string();
                        output.push_str(&format!("[error] {msg}\n"));
                        post_err = Some(msg);
                    }
                },
                Err(e) => {
                    let msg = format!("完成后命令启动失败: {e}");
                    output.push_str(&format!("[error] {msg}\n"));
                    post_err = Some(msg);
                }
            }
        }
        Ok(Some(expire))
    }
    .await;

    // ===== 汇总: 更新证书状态 + 写入颁发记录 =====
    let now = util::now_str();
    let (success, expire, mut message) = match result {
        Ok(exp) => (true, exp, "颁发成功".to_string()),
        Err(msg) => (false, None, msg),
    };
    if let Some(pe) = &post_err {
        message = format!("证书颁发成功，但完成后命令执行失败: {pe}");
    }

    let mut am = cert::ActiveModel {
        id: Set(cert.id),
        ..Default::default()
    };
    am.updated_at = Set(now.clone());
    am.last_attempt_at = Set(Some(now.clone()));
    if success {
        am.status = Set("issued".into());
        am.last_issued_at = Set(Some(now.clone()));
        am.cert_expire_at = Set(expire.clone());
        am.fail_count = Set(0);
        am.last_error = Set(post_err.clone().unwrap_or_default());
    } else {
        am.status = Set("failed".into());
        am.fail_count = Set(cert.fail_count + 1);
        am.last_error = Set(message.clone());
    }
    if let Err(e) = am.update(&state.db).await {
        tracing::error!("更新证书 #{cert_id} 状态失败: {e}");
    }

    let ptype = provider
        .as_ref()
        .map(|p| p.provider_type.clone())
        .unwrap_or_default();
    let pname = provider
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let rec = record::ActiveModel {
        cert_id: Set(cert.id),
        cert_name: Set(disp.clone()),
        trigger_type: Set(trigger.to_string()),
        issued_at: Set(now),
        expire_at: Set(expire.clone()),
        provider_type: Set(ptype),
        provider_name: Set(pname),
        domains: Set(cert.domains.clone()),
        cert_path: Set(cert.cert_path.clone()),
        key_path: Set(cert.key_path.clone()),
        ca: Set(cert.ca.clone()),
        success: Set(if success { 1 } else { 0 }),
        message: Set(util::trunc(&message, 500)),
        command_output: Set(util::trunc(&output, 60000)),
        ..Default::default()
    };
    if let Err(e) = rec.insert(&state.db).await {
        tracing::error!("写入颁发记录失败: {e}");
    }

    if success {
        tracing::info!("trigger={} 证书 #{} {} 颁发成功", trigger, cert_id, disp);
    } else {
        tracing::warn!(
            "trigger={} 证书 #{} {} 颁发失败: {}",
            trigger,
            cert_id,
            disp,
            message
        );
    }
}
