//! 统一的后台子进程启动辅助。
//!
//! Tauri 的 release 构建使用 `windows_subsystem = "windows"`（GUI 子系统，
//! 见 `main.rs`）。当 GUI 子系统进程启动控制台类子进程（如 `codex.exe`，
//! 或经由 `cmd.exe` 执行的 `codex.cmd` / `.bat` 批处理脚本）时，Windows
//! 会为子进程创建一个新的控制台窗口——即便调用方已经把 stdout/stderr 重定向
//! 到 `Stdio::null()` / 管道，窗口仍然会以空白形式弹出。
//!
//! [`spawn_without_window`] 在 Windows 上通过 `CREATE_NO_WINDOW` 创建标志禁止
//! 系统为子进程创建控制台窗口，同时完整保留 stdin/stdout/stderr 管道捕获、
//! 退出码与交互语义，也不影响 macOS / Linux（这些平台无此概念，原样返回）。

use std::process::Command;

/// 为“后台运行且无需用户看到终端”的 Windows 子进程隐藏控制台窗口。
///
/// 必须在 `Command` 上调用 `.spawn()` / `.status()` / `.output()` 之前设置。
/// 该函数是幂等的纯 `Command` 装饰器，便于在所有启动点统一复用，避免各处
/// 重复拼接 `creation_flags`。
#[cfg(target_os = "windows")]
pub fn spawn_without_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CreateProcess 的 CREATE_NO_WINDOW 标志（0x08000000）：进程作为“无控制台
    // 窗口”的控制台应用运行。它不会截断通过 STARTUPINFO 显式传入的标准句柄，
    // 因此 `Stdio::piped()` / `Stdio::null()` 的重定向与退出码行为保持不变。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// 非 Windows 平台不存在“控制台窗口”概念，无需任何处理。
#[cfg(not(target_os = "windows"))]
pub fn spawn_without_window(_command: &mut Command) {}
