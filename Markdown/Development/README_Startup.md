# AI API Monitor Windows 一键启动

## 使用方法

1. 确认本机已安装 Node.js 20.19 或更高版本、pnpm 11，以及 Rust（包含 Cargo）。
2. 首次运行前，在项目目录打开一次命令提示符并执行 `pnpm install`，安装前端依赖。
3. 双击项目根目录中的 `Start_AI_API_Monitor.bat`。
4. 脚本会自动进入项目目录，并执行项目实际的启动命令 `pnpm tauri dev`。Tauri 会自动启动 Vite 开发服务器。
5. 关闭应用后，启动窗口会显示退出状态；启动失败时请不要立即关闭窗口，以便查看错误信息。

脚本使用自身所在目录定位项目，因此项目路径包含空格时也可以正常工作。

## 常见启动错误

### 未找到 Node.js

安装 Node.js 20.19 或更高版本。安装完成后请重新打开 BAT 文件；如果刚安装仍提示缺失，请先重新登录 Windows 或重启资源管理器，使 PATH 更新生效。

### 未找到 pnpm

先安装 pnpm 11，或执行 `corepack enable` 后再重试。可以用 `pnpm --version` 验证安装是否成功。

### 未找到 Rust/Cargo

安装 Rustup（Windows 安装器），并确保 `cargo --version` 能在命令提示符中正常执行。Tauri 开发启动需要 Rust 工具链。

### 未找到 node_modules

在项目根目录执行：

```text
pnpm install
```

完成后再次双击 `Start_AI_API_Monitor.bat`。

### 端口 1420 已被占用

关闭正在运行的其他 AI API Monitor/Vite 进程，或在命令提示符中检查占用端口的进程：

```text
netstat -ano | findstr :1420
```

### Rust 编译失败或依赖下载失败

确认网络可用，并在项目根目录执行 `pnpm tauri dev` 查看完整错误。根据错误提示安装 Visual Studio Build Tools 的“使用 C++ 的桌面开发”组件，或更新 Rust 工具链。

### 窗口启动后立即退出

不要关闭 BAT 窗口，查看其中的错误信息。也可以在项目根目录手动执行 `pnpm tauri dev`，以便复制完整日志进行排查。
