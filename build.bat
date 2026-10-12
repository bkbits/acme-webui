@echo off
rem 一键构建: 前端 → 内嵌编译后端 → 产出单文件可执行程序
setlocal
cd /d "%~dp0"

echo ==^> [1/2] 构建前端 (vite-plus)
cd web
call bun install || exit /b 1
call bun run build || exit /b 1
cd ..

echo ==^> [2/2] 编译后端 (rust, 嵌入 web/dist)
cargo build --release || exit /b 1

echo.
echo 构建完成: target/release/acme-webui.exe
echo 启动示例: target/release/acme-webui.exe -h 0.0.0.0 -p 6980
endlocal
