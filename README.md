# ACME WebUI

acme 证书自动申请续期服务平台。通过 Web 界面维护证书配置（域名、证书位置、续期周期、完成后命令等），平台自动按周期调用 [acme.sh](https://github.com/acmesh-official/acme.sh) 命令行工具完成证书的颁发与续期，并完整记录每次颁发结果（成功与失败）。

## 功能特性

- **证书配置管理**：添加 / 修改 / 删除 / 批量删除（均带确认）；支持多域名与通配符（`*.example.com`）
- **自动周期续期**：距上次颁发（或添加）满 `续期周期(天)` 后由后台调度器自动颁发；也可随时「立即续期」
- **运营商配置**：覆盖 acme.sh 官方 wiki 全部 225 种 DNS 运营商（9 种主流 DNS + 216 种其他 DNS），菜单分为「主流 DNS / 其他 DNS」两个目录；配置直接以运营商命名（数据库保存权威英文名，菜单展示中文名），无需自定义名称；每个运营商一份配置，可启停（停用后相关证书暂停自动续期）
- **颁发记录**：记录每次颁发（含失败）的颁发时间、到期时间、运营商、域名、证书位置、CA、结果与完整命令输出；支持单条 / 批量删除（带确认）
- **完成后命令**：证书安装完成后自动执行（如 `nginx -s reload`），shell / cmd 均可
- **单文件部署**：前端资源在编译期嵌入二进制，一个可执行文件 + 一个 SQLite 数据库即可运行
- **Windows 支持（WSL）**：acme.sh 运行于 WSL 内，服务经 `wsl` 命令自动检测与调用；证书位置配置为 Windows 绝对路径，颁发成功后文件自动从 WSL 临时目录移动到 Windows 位置
- **主题切换**：亮色 / 暗色主题一键切换，选择自动记忆

## 技术栈

| 端 | 技术 |
| --- | --- |
| 后端 | Rust · tokio · axum 0.8 · sea-orm 1.x (sqlite3) · argon2 · rust-embed |
| 授权 | sa-token 风格会话层（`satoken` 请求头 + 滑动续期）¹ |
| 前端 | Vue 3 · vue-router 5 · antdv-next 1.6 · @lucide/vue · axios · scss |
| 工程 | bun（运行时/包管理）· vite-plus（`vp dev` / `vp build`）· vue-tsc |

> ¹ 说明：需求指定的 `satoken-rust` 截至 2026-10 尚未发布到 crates.io，本项目实现了等价的轻量会话层：UUID token、`satoken` 请求头（兼容 `Authorization: Bearer`）、7 天滑动续期、内存会话（重启后需重新登录）、argon2 密码哈希。

## 架构与颁发流程

```
 浏览器 (Vue3 SPA, 嵌入二进制)
    │  /api/*  (satoken 会话)
    ▼
 axum API ── sea-orm ──► SQLite (user / provider / cert / issue_record)
    │
    ├── 手动: POST /api/certs/{id}/renew ──► 后台任务
    └── 自动: 调度器 (默认 60s 扫描到期证书, CAS 抢占) ──► 后台任务 (全局并发=2)
                                              │
                                              ▼
        Unix:    acme.sh --issue --dns <hook> -d <域名> ... --server <CA> --keylength ec-256 --force
                 acme.sh --install-cert -d <主域名> --ecc --fullchain-file <公钥位置> --key-file <私钥位置>
        Windows: wsl <acme.sh 全路径> --issue ... --force --fullchain-file <WSL 临时目录> --key-file <WSL 临时目录>
                 wsl mv <WSL 临时目录> /mnt/<盘符>/<Windows 位置>   (证书不留在 WSL 内)
        完成后命令: sh -c "<cmd>" (Unix) / cmd /C "<cmd>" (Windows)   (私钥自动 chmod 600, Unix)
                                              │
                                              ▼
                             解析证书到期时间(x509) → 更新证书状态 → 写入颁发记录
```

- **续期周期语义**：`下次续期 = 上次颁发时间(或添加时间) + 续期周期`。添加后**不会**立即颁发，需点击「立即续期」或等到期自动颁发。
- **Windows (WSL) 查找顺序**：`ACME_WEBUI_ACME_SH`（WSL 内部路径 / `\\wsl$\<发行版>\...` / `C:\...` 盘符路径，均转为 WSL 内部路径验证）→ `wsl which ~/.acme.sh/acme.sh` → `wsl which acme.sh`；`ACME_WEBUI_WSL_DISTRO` 可指定发行版。
- **Windows 证书位置**：配置为 Windows 绝对路径（如 `C:/certs/public.pem`）；颁发成功后服务将其从 WSL 临时目录 `~/.certs/<主域名>-<证书ID>/` 移动到 Windows 位置（经 `/mnt/<盘符>` 映射），不留在 WSL 内。
- **失败重试**：自动续期失败后按 30min × 2ⁿ 指数退避重试（上限 24h），成功后重置；手动触发不受退避限制。
- **服务重启**：中断中的颁发任务会标记为失败并保留记录，到期未变的证书会在下个扫描周期自动重试。

## 环境要求

- 运行：Linux (x86_64/aarch64) / macOS / Windows（Windows 需 WSL，acme.sh 安装于 WSL 内），无其他运行时依赖（SQLite 已静态编译进二进制）
- 服务器安装 [acme.sh](https://github.com/acmesh-official/acme.sh)（Linux / macOS）：
  ```sh
  curl https://get.acme.sh | sh -s email=you@example.com
  ```
- Windows 安装 acme.sh（先装 WSL，再在 WSL 内安装）：
  ```powershell
  wsl --install   # PowerShell (管理员)
  ```
  ```sh
  # WSL (Ubuntu) 内
  curl https://get.acme.sh | sh -s email=you@example.com
  ```
- 未检测到 acme.sh 时 Web 界面会提示，点击可打开当前平台的安装指引（命令可一键复制，安装后可重新检测）
- 从源码构建：Rust ≥ 1.80（`rustup`）、bun ≥ 1.1

## 安装

### 直接安装（推荐）

一行命令安装最新版并注册为 systemd 服务（自动查询最新 Release、检测操作系统类型（linux / mac）与架构（amd64 / arm64），下载对应二进制）：

```sh
curl -fsSL https://raw.githubusercontent.com/bkbits/acme-webui/refs/heads/main/scripts/install.sh | sudo sh
```

脚本行为：

1. 查询最新 Release 的 tag，校验当前平台对应的资产存在（不存在时列出可用资产）
2. 下载二进制安装到 `/opt/acme-webui/acme-webui`
3. 写入 `/etc/systemd/system/acme-webui.service` 并 `enable`（无 systemd 的环境仅写入 unit 并提示）

安装完成后：

```sh
systemctl start acme-webui    # 启动服务（默认 0.0.0.0:6980）
systemctl status acme-webui   # 查看状态
```

- 数据库位于 `/opt/acme-webui/data/acme-webui.db`（首次启动自动创建），默认账号 `admin / admin123`（登录后请及时修改密码）
- 监听地址/端口可在安装时通过环境变量覆盖（经 `sudo env` 传入，避免被 sudo 重置环境）：
  ```sh
  curl -fsSL https://raw.githubusercontent.com/bkbits/acme-webui/refs/heads/main/scripts/install.sh | sudo env ACME_WEBUI_HOST=127.0.0.1 ACME_WEBUI_PORT=6980 sh
  ```
- 当前 Release 已发布 Linux 与 Windows 产物；安装脚本支持 Linux，macOS 产物暂未发布（可源码构建，见「快速开始」），Windows 直接从 [Release 页面](https://github.com/bkbits/acme-webui/releases) 下载 `.exe` 运行

## 快速开始

### 1. 源码运行（开发模式）

```sh
# 终端 1: 后端 (默认 6980 端口)
cargo run -- -h 127.0.0.1 -p 6980

# 终端 2: 前端 (5173 端口, /api 自动代理到 6980)
cd web
bun install
bun run dev
```

访问 http://127.0.0.1:5173 ，默认账号 `admin / admin123`（登录后请及时修改密码）。

### 2. 生产构建（单文件二进制）

```sh
# Linux / macOS
./build.sh
# 产物: target/release/acme-webui (已内嵌前端)
./target/release/acme-webui -h 0.0.0.0 -p 6980
```

```bat
rem Windows
build.bat
rem 产物: target\release\acme-webui.exe (已内嵌前端)
target\release\acme-webui.exe -h 0.0.0.0 -p 6980
```

### 3. 命令行参数

| 参数 | 说明 | 默认值 | 可选 |
| ---- | ---- | ------- | ---- |
| -h | 监听地址 | 0.0.0.0 | 是 |
| -p | 监听端口 | 6980 | 是 |

```sh
acme-webui -h 0.0.0.0 -p 6980
```

### 4. 环境变量

| 变量 | 说明 | 默认值 |
| ---- | ---- | ------- |
| `ACME_WEBUI_DB` | SQLite 数据库文件路径 | `data/acme-webui.db` |
| `ACME_WEBUI_ACME_SH` | 自定义 acme.sh 路径（Windows 下 acme.sh 须位于 WSL 内：可填 WSL 内部路径、`\\wsl$\<发行版>\...` 形式路径或 `C:\...` 盘符路径，均会转为 WSL 内部路径验证） | Unix 依次探测 `~/.acme.sh/acme.sh` → `PATH`；Windows 在 WSL 发行版内依次探测 `~/.acme.sh/acme.sh` → `PATH`（which） |
| `ACME_WEBUI_WSL_DISTRO` | WSL 发行版名称（仅 Windows 生效，指定检测/调用 acme.sh 的发行版） | 默认发行版 |
| `ACME_WEBUI_ADMIN_PASSWORD` | 首次启动创建 admin 时的密码 | `admin123` |
| `ACME_WEBUI_SCAN_INTERVAL` | 调度器扫描间隔（秒） | `60` |

### 5. systemd 示例

```ini
[Unit]
Description=ACME WebUI
After=network.target

[Service]
WorkingDirectory=/opt/acme-webui
ExecStart=/opt/acme-webui/acme-webui -h 0.0.0.0 -p 6980
Restart=always
# 运行用户需有权限写入证书目标路径并执行完成后命令
User=root

[Install]
WantedBy=multi-user.target
```

## 使用说明

### 运营商凭据获取指引

菜单分为「主流 DNS」（9 种）与「其他 DNS」（216 种）两个目录，覆盖 acme.sh 官方 wiki 全部运营商。每个运营商的配置页展示所需凭据字段（字段名即 acme.sh 环境变量名）与对应的 wiki 文档链接，并支持「其他环境变量」自由补充。

常用主流运营商凭据参考：

| 运营商 | 凭据获取位置 | 所需权限 / 说明 |
| ------ | ------------ | -------------- |
| 阿里云 | RAM 访问控制 → AccessKey 管理 | 云解析 DNS（AliyunDNSFullAccess，建议子账号） |
| Cloudflare | My Profile → API Tokens（推荐），或 Global API Key + 邮箱 | Zone - DNS - Edit |
| AWS Route53 | IAM 用户 Access Key | Route 53 记录变更权限；也可用本机 AWS CLI 凭据（无需填写） |
| Azure | Entra ID (Azure AD) 应用注册 + 客户端密码 | DNS 区权限，需 Subscription ID / Tenant ID / App ID |
| 华为云 (AK/SK) | IAM 访问密钥 | DNS 操作权限 |

凭据以环境变量形式传给 acme.sh（与官方 DNS API 插件一致），如：阿里云 `Ali_Key/Ali_Secret`、Cloudflare `CF_Token(/CF_Account_ID)` 或 `CF_Email+CF_Key`、AWS `AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY`、华为云 (AK/SK) `HW_AK/HW_SK(/HW_Region)`；各运营商完整字段以配置页展示为准。

### 添加证书

1. 前往「运营商配置」（「主流 DNS / 其他 DNS」目录下选择对应运营商）填写凭据并保存
2. 「证书管理」→「添加证书」，填写域名（回车添加，支持多个与 `*.example.com`）、公钥/私钥位置（绝对路径）、CA 机构（默认 Let's Encrypt）、续期周期（天）、完成后命令（可选）
3. 点击行内「立即续期」立即颁发，或等待到达续期时间后自动颁发
4. 颁发中的证书状态为 `颁发中`，完成后可在「颁发记录」查看详情（含完整 acme.sh 输出）

### 平台执行的命令

```sh
# Unix
# 1) 颁发（--force 由平台自行控制周期，ECC P-256）
acme.sh --issue --dns dns_ali -d test.example.com -d '*.example.com' \
  --server letsencrypt --keylength ec-256 --force
# 2) 安装到配置的位置
acme.sh --install-cert -d test.example.com --ecc \
  --fullchain-file /root/certs/public.pem --key-file /root/certs/private.key
# 3) 执行完成后命令
sh -c "nginx -s reload"
```

```bat
rem Windows（服务自动调用 WSL，无需手动执行）
rem 1) 颁发，写入 WSL 临时目录（避免直写 /mnt 映射盘）
wsl <acme.sh 全路径> --issue --dns dns_ali -d test.example.com -d *.example.com --server letsencrypt --keylength ec-256 --force --fullchain-file ~/.certs/test.example.com-1/public.pem --key-file ~/.certs/test.example.com-1/private.key
rem 2) 移动到配置的 Windows 位置（示例 C:\root\certs）
wsl -e bash -c "mkdir -p /mnt/c/root/certs && mv ~/.certs/test.example.com-1/public.pem /mnt/c/root/certs/public.pem && mv ~/.certs/test.example.com-1/private.key /mnt/c/root/certs/private.key && rm -rf ~/.certs/test.example.com-1"
rem 3) 执行完成后命令
cmd /C "nginx -s reload"
```

> 提示：acme.sh 安装时自带的 cron 与本平台互不冲突（本平台始终带 `--force` 独立颁发）。如担心重复续期，可 `crontab -e` 移除 acme.sh 的定时任务。证书源文件仍保存在 `~/.acme.sh/` 中，可随时手动恢复。

## 开发与测试

```sh
# 前端类型检查 / 构建
cd web && bun run typecheck && bun run build

# 后端
cargo build --release

# E2E 测试（使用 mock acme.sh, 不请求真实 CA/DNS）
ACME_WEBUI_DB=/tmp/t.db ACME_WEBUI_ACME_SH=$PWD/scripts/mock-acme.sh \
  ACME_WEBUI_SCAN_INTERVAL=3 E2E_DB=/tmp/t.db ./target/release/acme-webui -p 16980 &
python3 scripts/e2e_test.py http://127.0.0.1:16980
```

`scripts/mock-acme.sh` 模拟 acme.sh 行为（域名含 `fail` 时模拟失败），配合 `ACME_WEBUI_ACME_SH` 环境变量可在无真实凭据的情况下本地体验完整流程。

## 目录结构

```
acme-webui/
├── Cargo.toml              # Rust 依赖
├── build.sh                # 一键构建（前端→嵌入→后端, Linux/macOS）
├── build.bat               # 一键构建（Windows）
├── src/
│   ├── main.rs             # 启动入口（-h/-p 参数、初始化、调度器）
│   ├── db.rs               # SQLite 初始化 + DDL + 默认账号 + 配置迁移
│   ├── auth.rs             # 会话管理 / argon2 / AuthUser 提取器
│   ├── registry.rs         # DNS 运营商注册表（225 种: 9 主流 + 216 其他）+ CA 列表（单一事实来源）
│   ├── error.rs / util.rs / state.rs / static_files.rs
│   ├── entities/           # sea-orm 实体: user/provider/cert/record
│   ├── api/                # axum 路由: auth/certs/providers/records/meta
│   └── services/           # acme.sh 调用（含 Windows/WSL）+ 周期调度器
├── web/
│   ├── package.json / vite.config.ts / tsconfig.json
│   └── src/
│       ├── api/            # axios 封装 + 类型定义
│       ├── router/ composables/ layouts/ views/ components/
│       └── styles/index.scss
└── scripts/
    ├── install.sh          # 一键安装脚本（最新 Release + systemd 服务）
    ├── mock-acme.sh        # acme.sh 模拟器（测试用）
    └── e2e_test.py         # 端到端测试（43 项断言）
```

## 常见问题

**Q: 添加证书后多久会颁发？**
不会立即颁发。到期时间 = 添加时间 + 续期周期（天）；需要立即颁发请点击「立即续期」。

**Q: 颁发失败怎么办？**
在证书列表悬停「失败」状态查看原因，并在「颁发记录 → 详情」查看完整命令输出。自动续期会指数退避重试；常见原因：凭据权限不足、DNS 未接入对应运营商、acme.sh 未安装、目标路径无写权限。

**Q: 主域名相同的多条证书配置？**
acme.sh 以主域名（第一个域名）命名证书目录，同一主域名的多条配置会复用同一目录，建议保证各配置主域名唯一。

**Q: 会话有效期？**
7 天滑动续期；服务重启后需重新登录（会话保存在内存中）。

**Q: 数据备份？**
备份 `data/acme-webui.db`（及 WAL/SHM 文件）即可；证书源文件在 `~/.acme.sh/`。

**Q: 支持哪些 CA？**
Let's Encrypt（默认）、Let's Encrypt 测试环境、ZeroSSL、Buypass、Google Trust Services。新增 CA 只需在 `src/registry.rs` 的 `CA_LIST` 中添加映射。

**Q: DNS 运营商怎么选？**
左侧菜单分「主流 DNS」（9 种常用）与「其他 DNS」（216 种）两个目录，覆盖 acme.sh 官方 wiki 全部运营商。从菜单选择后在配置页填写，页内提供 wiki 文档链接与「其他环境变量」自由补充。

**Q: Windows 下如何使用？**
需 WSL（推荐 Ubuntu）。acme.sh 安装在 WSL 内，服务自动检测（`wsl which`）并调用；证书公钥/私钥位置配置为 Windows 绝对路径（如 `C:/certs/fullchain.pem`），颁发成功后服务将文件从 WSL 临时目录 `~/.certs/` 移动到 Windows 位置，不留在 WSL 内。`ACME_WEBUI_WSL_DISTRO` 可指定 WSL 发行版。

**Q: 如何新增 DNS 运营商？**
acme.sh 官方 wiki 的 225 种类型已预注册；新增运营商需在 `src/registry.rs` 的 `PROVIDER_TYPES` 中添加类型（key/label/dns_hook/category/凭据字段/wiki 链接），前端菜单与表单自动适配，无需改动前端代码。
