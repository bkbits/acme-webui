#!/usr/bin/env bash
# 一键构建: 前端 → 内嵌编译后端 → 产出单文件可执行程序
set -e
cd "$(dirname "$0")"

echo "==> [1/2] 构建前端 (vite-plus)"
cd web
bun install
bun run build
cd ..

echo "==> [2/2] 编译后端 (rust, 嵌入 web/dist)"
cargo build --release

echo ""
echo "构建完成: target/release/acme-webui"
echo "启动示例: ./target/release/acme-webui -h 0.0.0.0 -p 6980"
