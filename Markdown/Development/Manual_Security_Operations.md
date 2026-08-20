# AI API Monitor 手工安全操作手册

适用项目：AI API Monitor 项目根目录
适用版本：Tauri 2 / React 19 / Rust 1.97.1  
执行原则：任何私钥、证书密码、API Key 都不得粘贴到聊天、Issue、日志或提交到 Git。

## 一、每次修改或发布前的本地门禁

在 PowerShell 中进入项目：

```powershell
Set-Location "<项目根目录>"
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

成功标准：

- TypeScript 无错误。
- Vitest 9 项全部通过。
- Rust 80 项全部通过。
- Clippy 没有 warning/error。
- `dist` 生产构建成功。

不要在测试失败时继续发布。

## 二、执行 JavaScript 与 Rust 依赖漏洞审计

### 2.1 安装 RustSec 工具（每台开发机只做一次）

```powershell
cargo install cargo-audit --locked
cargo audit --version
```

### 2.2 执行统一审计

```powershell
Set-Location "<项目根目录>"
pnpm security:audit
```

成功标准：命令退出码为 `0`，JavaScript 报告 `No known vulnerabilities found`，RustSec 不报告 vulnerability。

如果发现漏洞：

1. 记录 advisory ID、受影响包和修复版本。
2. 只升级对应依赖，不要直接执行不受控的强制修复。
3. 重新执行 `pnpm install`、`pnpm check`、`pnpm build` 和 `pnpm security:audit`。
4. 如果只能暂时接受风险，在 `Security_Audit_Report.md` 记录原因、影响范围、补偿措施和到期日期。

## 三、首次启用安全自动更新

> Tauri 更新签名密钥与 Windows/macOS 应用发布证书不是同一种东西。更新签名密钥验证更新包完整性；平台代码签名用于证明发布者身份。正式发布通常两者都需要。

### 3.1 先确认首次部署策略

当前版本的 `src-tauri/tauri.conf.json` 中公钥和更新地址为空。已安装这个版本的用户无法通过自动更新获得新公钥。

- 尚无外部用户：直接制作一个包含正式公钥的基线安装包，再从该版本测试升级。
- 已有外部用户：先让用户手动安装一次包含正式公钥的新基线版本；之后的版本才能自动更新。

### 3.2 在项目目录之外生成更新签名密钥（只做一次）

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.tauri" | Out-Null
pnpm tauri signer generate -w "$env:USERPROFILE\.tauri\ai-api-monitor.key"
```

生成时设置一个强密码。确认以下文件存在：

```powershell
Get-ChildItem "$env:USERPROFILE\.tauri\ai-api-monitor.key*"
```

操作规则：

- `ai-api-monitor.key` 是私钥，绝不能放进项目、Git、网盘公开链接或 Release。
- 公钥可以公开，并需要把“公钥内容”写入配置；不能写公钥文件路径。
- 私钥和密码分别备份到两个安全位置，例如密码管理器和加密离线介质。
- 丢失私钥后，现有安装无法验证由新密钥签发的更新。

建议在 `.gitignore` 加入：

```gitignore
# Release signing secrets
*.key
*.p12
*.pfx
*.pem
```

提交前检查私钥没有进入仓库：

```powershell
git status --short
git ls-files | Select-String -Pattern '\.(key|p12|pfx|pem)$'
```

第二条命令应无输出。

### 3.3 读取公钥

```powershell
Get-Content "$env:USERPROFILE\.tauri\ai-api-monitor.key.pub" -Raw
```

只复制公钥内容，不复制私钥内容。

### 3.4 修改 Tauri 更新配置

编辑 `src-tauri/tauri.conf.json`，在 `bundle` 中增加 `createUpdaterArtifacts`，并填写更新器公钥和 HTTPS 地址：

```json
{
  "plugins": {
    "updater": {
      "pubkey": "这里放完整公钥内容，不是文件路径",
      "endpoints": [
        "https://github.com/你的账号/你的仓库/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true
  }
}
```

注意：

- 更新地址必须为 HTTPS。
- 不要把 Token、用户名或密码放入 URL。
- 私有 GitHub Release 通常需要认证，不适合作为当前无认证更新客户端的直接地址；使用公开 Release 或公开 HTTPS/CDN。
- `latest.json` 应禁止长期缓存；版本安装包应使用不可变的版本化文件名。
- Windows 使用 `passive`；不要使用不显示进度且可能无法提权的 `quiet`。

### 3.5 同步版本号

例如准备发布 `0.1.1`，同步修改：

- `src-tauri/tauri.conf.json` 的 `version`
- `src-tauri/Cargo.toml` 的 `[package].version`
- `package.json` 的 `version`

版本必须是合法 SemVer，例如 `0.1.1`，不能使用 `latest`、日期字符串或比当前版本更低的版本。

检查：

```powershell
Select-String -Path src-tauri\tauri.conf.json,src-tauri\Cargo.toml,package.json -Pattern 'version'
```

### 3.6 使用私钥构建更新包

不要把私钥或密码写入 `.env`。在当前 PowerShell 会话临时设置：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY="$env:USERPROFILE\.tauri\ai-api-monitor.key"
$securePassword = Read-Host "更新签名私钥密码" -AsSecureString
$passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
try {
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)
  pnpm tauri build
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPointer)
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
  $securePassword = $null
}
```

列出构建产物：

```powershell
Get-ChildItem src-tauri\target\release\bundle -Recurse -File |
  Select-Object FullName,Length,LastWriteTime
```

Windows 应至少看到安装程序和同名 `.sig` 文件。根据实际选择 NSIS `.exe` 或 MSI；后续 JSON 中的 URL 和签名必须对应同一个文件。

### 3.7 制作静态 `latest.json`

读取对应签名文件完整内容：

```powershell
Get-Content "安装包的完整路径.sig" -Raw
```

创建 `latest.json`：

```json
{
  "version": "0.1.1",
  "notes": "安全加固与稳定性更新",
  "pub_date": "2026-08-13T15:00:00+08:00",
  "platforms": {
    "windows-x86_64": {
      "signature": "粘贴.sig文件的完整内容",
      "url": "https://github.com/你的账号/你的仓库/releases/download/v0.1.1/版本化安装包.exe"
    }
  }
}
```

要求：

- `signature` 必须是 `.sig` 文件内容，不能是文件路径或 `.sig` 下载 URL。
- `url` 必须指向这次签名的原始安装包。
- `pub_date` 使用 RFC 3339。
- 同时发布其他平台时增加 `linux-x86_64`、`darwin-x86_64`、`darwin-aarch64` 等实际构建目标。

### 3.8 安全上传顺序

1. 先上传版本化安装包。
2. 下载一次并核对 SHA-256：

```powershell
Get-FileHash "本地安装包路径" -Algorithm SHA256
```

3. 确认线上文件可下载且 SHA-256 与本地一致。
4. 最后上传/替换 `latest.json`。
5. 不要覆盖旧版本安装包；只更新 `latest.json` 指针。

检查更新文件：

```powershell
Invoke-RestMethod "https://你的实际地址/latest.json" | ConvertTo-Json -Depth 5
```

## 四、自动更新必须完成的四项验收

使用独立测试更新地址或测试 Release，不要破坏生产 `latest.json`。

### 4.1 正常升级

1. 安装包含正式公钥的旧基线版本，例如 `0.1.0`。
2. 发布签名正确的 `0.1.1`。
3. 在应用中检查更新并安装。
4. 重启后确认版本变成 `0.1.1`，配置、Provider 和图片仍然存在。

### 4.2 错误签名

1. 复制测试用 `latest.json`。
2. 只修改 `signature` 中一个字符。
3. 让测试版指向该 JSON。
4. 预期：下载或安装失败，应用不能被替换。

### 4.3 被篡改安装包

1. 在测试发布中复制安装包，并修改复制品的任意字节。
2. 保持 JSON 中原签名不变，让 URL 指向被修改的复制品。
3. 预期：签名验证失败，不能安装。
4. 不要修改或覆盖生产安装包进行此测试。

### 4.4 降级与重复安装

1. 在已安装 `0.1.1` 的机器上，让测试 JSON 返回 `0.1.1` 或 `0.1.0`。
2. 预期：应用不允许重复安装或降级。
3. 再把测试 JSON 改回更高版本，确认升级仍正常。

四项全部通过后，才可将生产 `latest.json` 指向新版本。

## 五、Windows 本机数据安全验收

### 5.1 创建测试凭据

1. 启动安装版应用。
2. 新增测试 Provider，使用专用假 Key：`sk-security-canary-ABCD1234`。
3. UI 应只显示类似 `sk-****1234`，不能显示完整值。

不要用真实生产 Key 做安全测试。

### 5.2 检查 Windows Credential Manager

```powershell
control.exe /name Microsoft.CredentialManager
```

打开“Windows 凭据/通用凭据”，查找服务 `com.aiapimonitor.desktop`。应看到：

- `data_encryption_key_v1`
- 一个或多个 `key_<UUID>` 账户

只能确认条目存在，不要展示、截图或复制凭据值。删除应用中的测试 Provider 后，对应 `key_<UUID>` 条目应被删除。

### 5.3 找到并检查应用数据目录

通常位于：

```powershell
$dataRoot = Join-Path $env:APPDATA "com.aiapimonitor.desktop"
Get-ChildItem $dataRoot -Force
```

如果目录不存在：

```powershell
Get-ChildItem $env:APPDATA -Directory |
  Where-Object Name -Match 'aiapi|monitor'
```

检查权限：

```powershell
icacls $dataRoot
icacls (Join-Path $dataRoot "ai-api-monitor.db")
icacls (Join-Path $dataRoot "assets")
icacls (Join-Path $dataRoot "logs")
```

成功标准：目录和文件没有 `Everyone`、`Users`、`Authenticated Users` 的读取授权，只应由对象所有者和 `SYSTEM` 控制。若应用放在公司重定向配置目录，还要确认域管理员策略没有额外开放访问。

### 5.4 检查 SQLite 不含完整 Key/Token/Cookie

如果已安装 `sqlite3`：

```powershell
$db = Join-Path $dataRoot "ai-api-monitor.db"
sqlite3 $db ".schema providers"
sqlite3 $db "SELECT id,name,provider_type,api_url,key_ref,key_hint FROM providers;"
sqlite3 $db "SELECT key,length(ciphertext),length(nonce) FROM secure_settings;"
sqlite3 $db "SELECT key,value FROM settings WHERE lower(key) LIKE '%token%' OR lower(key) LIKE '%cookie%' OR lower(key) LIKE '%password%' OR lower(key) LIKE '%secret%' OR lower(key) LIKE '%key%';"
```

成功标准：

- `providers` 只有 `key_ref` 和掩码 `key_hint`，没有完整 Key。
- `secure_settings` 只有密文和 nonce。
- `settings` 不出现明文 Token、Cookie、Password、Secret 或 API Key。

### 5.5 搜索日志和应用数据中的测试 Key

```powershell
$canary = 'sk-security-canary-ABCD1234'
Get-ChildItem $dataRoot -Recurse -File -ErrorAction SilentlyContinue |
  Where-Object Extension -In '.log','.txt','.json' |
  Select-String -SimpleMatch $canary
```

成功标准：无输出。随后在应用中删除测试 Provider，并再次检查 Credential Manager。

### 5.6 检查隐私与网络行为

1. 确认设置中的 Telemetry 默认为关闭。
2. 使用 Windows TCPView、资源监视器或 Wireshark 观察应用连接。
3. 空闲且未刷新 Provider 时，不应连接统计、广告或遥测域名。
4. 刷新 Provider 时，只应连接已配置的官方 Provider 或你明确批准的自定义网关。
5. 导入图片/GIF 时不应产生上传连接。

## 六、Codex 登录隔离手工验收

1. 准备一台已登录 Codex CLI 的测试机器。
2. 在终端执行 `codex login status`，确认该公开状态命令正常。
3. 在应用中添加 Codex Provider；UI 不应要求或显示 API Key。
4. 刷新后只应显示登录/可用状态，不应出现 Token、Cookie 或认证正文。
5. 检查应用日志中没有 `access_token`、`refresh_token`、`Bearer`、Cookie 值。
6. 使用 Process Monitor 时只过滤 AI API Monitor 主进程；官方 `codex` 子进程可能为了实现自己的公开状态命令读取其自身配置，这不代表应用读取浏览器 Cookie。
7. 主进程不得访问 Chrome、Edge、Safari Cookie 数据库或浏览器 Local/Session Storage。

## 七、图片/GIF 隔离手工验收

分别尝试导入：

- 正常 PNG/JPG/JPEG/WEBP/GIF/ICO/SVG。
- 大于 20MB 的 GIF。
- 大于 4096×4096 的图片。
- 301 帧 GIF。
- 改成图片扩展名的 EXE/HTML/JS。
- 含 `<script>`、事件属性、外部引用或 `javascript:` 的 SVG。

成功标准：

- 只有安全白名单文件可导入。
- 超限或活动内容被拒绝。
- 导入后文件复制到 `$dataRoot\assets`，文件名为随机 ID。
- 关闭原始文件所在磁盘或移动原文件后，应用图片仍正常。
- UI/日志中不出现原始绝对路径或 `file://`。

## 八、macOS Keychain 与权限验收

必须在真实 macOS 图形登录会话中执行。

```bash
pnpm install --frozen-lockfile
pnpm check
pnpm tauri build
```

安装并运行应用，新增专用假 Key。然后：

```bash
security find-generic-password -s com.aiapimonitor.desktop
stat -f '%Sp %N' "$HOME/Library/Application Support/com.aiapimonitor.desktop"
stat -f '%Sp %N' "$HOME/Library/Application Support/com.aiapimonitor.desktop/ai-api-monitor.db"
```

成功标准：

- Keychain 中存在 `com.aiapimonitor.desktop`，但检查时不要使用 `-w` 输出密码。
- 目录权限为 `drwx------`，数据库为 `-rw-------`。
- 删除测试 Provider 后，对应 Keychain 项被删除。
- 应用重启后仍能读取凭据；锁定 Keychain 时应安全失败，不能回退到明文保存。

正式分发 macOS 版本还需要 `Developer ID Application` 证书和 Apple notarization；Ad-Hoc 签名不能替代正式身份与公证。

## 九、Linux Secret Service 与权限验收

必须在带 DBus 用户会话和 Secret Service 的真实桌面环境中执行。Ubuntu/Debian 可先安装：

```bash
sudo apt update
sudo apt install gnome-keyring libsecret-tools dbus-user-session
```

登录图形桌面后确认 Secret Service 可用，再安装运行应用并新增专用假 Key：

```bash
secret-tool search service com.aiapimonitor.desktop
DATA_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/com.aiapimonitor.desktop"
stat -c '%A %a %n' "$DATA_ROOT"
stat -c '%A %a %n' "$DATA_ROOT/ai-api-monitor.db"
```

成功标准：

- Secret Service/Seahorse 中存在服务项；不要使用会输出 secret 的查询参数。
- 目录权限是 `700`，数据库是 `600`。
- 删除 Provider 后对应凭据消失。
- Secret Service 锁定或不可用时，应用应显示安全错误，不能把 Key 写入 SQLite/JSON。

如果终端能运行而桌面图标启动失败，检查图形会话的 DBus/Secret Service，不要改成明文文件作为“修复”。

## 十、CI 与发布最终检查

推送代码后打开 GitHub Actions，确认 `Quality` 工作流中的两个 Job 都为绿色：

- `check`
- `security-audit`

正式发布前执行：

```powershell
Set-Location "<项目根目录>"
git diff --check
git status --short
pnpm check
pnpm build
pnpm security:audit
```

人工确认：

- Git 中没有 `.key`、`.p12`、`.pfx`、`.pem`、真实 API Key、Token、Cookie。
- `latest.json` 的版本高于当前版本。
- 安装包 URL 与 `.sig` 内容完全对应。
- Windows/macOS 发布包完成平台代码签名；macOS 完成 notarization。
- 发布后从一台干净机器执行完整安装和一次真实升级。
- `Security_Test_Report.md` 填写本次版本、日期、平台和验收结果。

## 十一、出现问题时立即停止发布的条件

以下任一情况出现时不要继续发布：

- UI、SQLite、JSON 或日志中出现完整 Key/Token/Cookie。
- 更新包错误签名或被篡改后仍能安装。
- 应用允许降级。
- 自定义 Provider 可连接 localhost、IP 地址或私网目标。
- 图片导入后仍依赖用户原始路径或出现 `file://`。
- Credential Manager/Keychain/Secret Service 不可用时回退为明文存储。
- `pnpm check`、`pnpm build`、`pnpm security:audit` 或 CI 任一失败。

停止后保留失败日志的脱敏副本，不要附带真实凭据；修复并重新执行整套门禁。
