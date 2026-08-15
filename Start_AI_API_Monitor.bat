@echo off
setlocal
chcp 65001 >nul
title AI API Monitor 启动器

echo ========================================
echo        AI API Monitor 一键启动器
echo ========================================
echo.

cd /d "%~dp0"
if errorlevel 1 goto :cd_failed

where node >nul 2>&1
if errorlevel 1 goto :node_missing

where pnpm >nul 2>&1
if errorlevel 1 goto :pnpm_missing

where cargo >nul 2>&1
if errorlevel 1 goto :rust_missing

if exist "package.json" goto :package_ok
goto :project_missing

:package_ok
if exist "src-tauri\Cargo.toml" goto :tauri_ok
goto :project_missing

:tauri_ok
if exist "node_modules" goto :environment_ok

:deps_missing
echo.
echo [环境初始化] 未找到 node_modules，正在安装项目依赖...
echo 将按 pnpm-lock.yaml 锁定版本安装：pnpm install --frozen-lockfile
echo 首次安装可能需要几分钟，请稍候。
echo.
call pnpm install --frozen-lockfile
if errorlevel 1 (
    echo.
    echo [错误] 项目依赖安装失败。
    echo 请检查上方 pnpm 错误信息（网络、镜像源或版本冲突），修复后重新运行本脚本。
    goto :pause_error
)
echo.
echo [成功] 项目依赖安装完成。
echo.

:environment_ok
echo [检查通过] 项目目录：%CD%
echo [检查通过] Node.js、pnpm、Rust/Cargo 和前端依赖已就绪。
echo.
echo 正在启动 AI API Monitor...
echo 关闭应用后，本窗口会显示退出状态；如启动失败，请保留窗口中的错误信息。
echo.
call pnpm tauri dev
set "EXIT_CODE=%ERRORLEVEL%"
echo.
if "%EXIT_CODE%"=="0" goto :normal_exit
echo [启动失败] AI API Monitor 退出代码：%EXIT_CODE%
echo 请根据上方错误信息处理后重试。
pause
exit /b %EXIT_CODE%

:normal_exit
echo [已退出] AI API Monitor 已正常关闭。
pause
exit /b 0

:cd_failed
echo [错误] 无法进入项目目录：%~dp0
goto :pause_error

:node_missing
echo [缺少环境] 未找到 Node.js。请安装 Node.js 20.19 或更高版本，并重新打开此脚本。
goto :pause_error

:pnpm_missing
echo [缺少环境] 未找到 pnpm。请先安装 pnpm 11（或使用 npm 启用 Corepack），然后重试。
goto :pause_error

:rust_missing
echo [缺少环境] 未找到 Rust/Cargo。请安装 Rust（推荐 rustup），然后重试。
goto :pause_error

:project_missing
echo [项目错误] 当前目录不是完整的 AI API Monitor 项目：%CD%
goto :pause_error

:pause_error
echo.
pause
exit /b 1
