> 历史原稿：本文件已被 `./Security_Audit_Report.md`、`./Security_Test_Report.md` 及当前 Rust 安全实现替代。保留它用于追溯早期安全设计，不作为当前实现状态依据。

针对 AI API Monitor，这类软件本质上属于**高敏感桌面应用**：

- 存储 AI 平台 API Key
- 可能读取 Codex / OpenAI 登录状态
- 访问第三方 API
- 长时间后台运行
- 处理用户导入图片/GIF/主题资源
- 可能拥有悬浮窗、托盘、自启动权限

因此安全设计应该按照「本地凭据保护 + 数据隔离 + 网络安全 + 文件安全 + UI 安全 + 系统权限控制」几个方向设计。

下面是完整安全方案，并附最后给 Codex 的开发 Prompt。

------

# AI API Monitor 安全保障方案

## 1. API Key 与登录凭据安全

### 风险

用户可能添加：

- OpenAI API Key
- DeepSeek API Key
- Anthropic API Key
- Gemini API Key
- 其他 Provider Token

风险：

- 明文保存到配置文件
- 日志打印 Key
- 崩溃报告泄露
- UI 显示完整 Key
- 第三方插件读取
- 恶意软件扫描本地文件

------

## 方案

### 1.1 禁止明文保存 API Key

不要：

```
{
 "api_key":"sk-xxxxxxxx"
}
```

改为：

Windows：

使用：

```
Windows Credential Manager
```

macOS：

使用：

```
Keychain
```

统一封装：

```
SecureCredentialManager
```

结构：

```
AI API Monitor

Credential Layer

    |
    |
Windows Credential Manager
macOS Keychain
Linux Secret Service
```

------

### 1.2 API Key 加密存储

如果必须本地保存：

采用：

```
AES-256-GCM
```

密钥来源：

Windows:

```
DPAPI
```

macOS:

```
Keychain-derived key
```

不要：

- 固定密码
- 写死密钥
- 存在源码

------

### 1.3 UI显示脱敏

禁止：

```
sk-proj-abc123456789
```

显示：

```
sk-proj-****6789
```

支持：

点击查看：

```
长按3秒显示
```

或者：

```
重新输入系统密码验证
```

------

# 2. Codex 登录安全

## 风险

如果用户使用：

- ChatGPT 登录
- Codex 登录
- OAuth 登录

可能存在：

```
cookie
session token
refresh token
access token
```

泄露风险。

例如：

错误：

```
读取 ~/.codex/auth.json
上传服务器
打印日志
复制到剪贴板
```

这是严重安全问题。

------

# 方案

## 2.1 禁止读取浏览器 Cookie

明确禁止：

扫描：

```
Chrome Cookies
Edge Cookies
Safari Cookies
Firefox Cookies
```

禁止：

```
Login Data
Cookies
Local Storage
Session Storage
```

目录：

```
Chrome/User Data/
Edge/User Data/
```

全部排除。

------

## 2.2 Codex 登录采用隔离读取

如果需要检测 Codex：

只允许：

读取公开状态信息：

例如：

```
codex version
account status
usage endpoint
```

禁止：

获取：

```
cookie
refresh_token
access_token
session_id
```

------

## 2.3 Token 生命周期保护

所有：

```
access token
refresh token
OAuth token
```

禁止：

- 输出日志
- 显示 UI
- 导出
- 复制
- 上传

日志：

错误：

```
Login failed token=xxxxx
```

正确：

```
Login failed authentication error
```

------

# 3. 防止图片/GIF资源泄露

## 风险

用户可能导入：

- 头像
- UI图片
- GIF动画
- 自定义主题

风险：

恶意程序：

- 读取路径
- 上传文件
- 注入网页
- 拖入浏览器
- 路径泄露

------

# 方案

## 3.1 文件沙箱

导入资源复制到：

```
AppData/
 AIAPIMonitor/
    Assets/
```

不要直接引用：

```
D:\Personal\Pictures\a.gif
```

------

## 3.2 文件类型白名单

允许：

```
png
jpg
jpeg
webp
gif
ico
svg
```

禁止：

```
exe
bat
cmd
dll
js
html
hta
```

------

## 3.3 图片加载隔离

禁止：

图片路径：

直接进入：

```
BrowserView
WebView
HTML
```

避免：

```
file://C:/xxx/image.gif
```

泄露。

使用：

内部资源协议：

例如：

```
app-resource://asset/12345
```

------

## 3.4 GIF安全处理

GIF可能包含：

- 超大尺寸
- 解压炸弹
- 无限循环

限制：

最大：

```
20MB
```

尺寸：

```
4096x4096
```

帧数：

```
300 frames
```

超过：

拒绝加载。

------

# 4. 网络安全

## 风险

API请求可能：

- 中间人攻击
- DNS污染
- HTTPS降级
- 代理劫持

------

# 方案

## 4.1 强制HTTPS

禁止：

```
http://api.xxx.com
```

必须：

```
https://
```

------

## 4.2 TLS验证

禁止：

```
verify=False
```

禁止：

忽略证书错误。

------

## 4.3 API Endpoint白名单

不要允许用户输入：

```
http://evil.com/api
```

默认：

```
OpenAI
DeepSeek
Claude
Gemini
```

采用：

Provider Registry。

------

## 4.4 防止请求泄露

HTTP日志禁止：

打印：

```
Authorization:
Bearer xxxx
```

改：

```
Authorization:
Bearer ****
```

------

# 5. 防止数据外泄

## 风险

软件可能上传：

- 使用记录
- API余额
- Key
- 图片
- 用户配置

------

# 方案

默认：

```
Zero Telemetry
```

不开启统计。

如果未来增加：

必须：

设置：

```
隐私模式
```

用户明确开启。

------

# 6. 本地数据库安全

如果使用：

SQLite：

不要：

```
database.db
```

明文保存。

方案：

加密数据库：

```
SQLCipher
```

或者：

字段级AES。

------

保存：

允许：

```
Provider名称
刷新时间
余额缓存
```

禁止：

```
完整Token
完整Key
登录Cookie
```

------

# 7. 日志安全

日志过滤：

创建：

```
SensitiveDataFilter
```

自动过滤：

匹配：

```
sk-
Bearer
token
cookie
session
password
secret
key
```

例如：

输入：

```
Authorization Bearer sk-123456
```

输出：

```
Authorization Bearer ******
```

------

# 8. 悬浮窗安全

风险：

悬浮窗可能：

- 截屏泄露
- 被录屏软件捕获
- 显示敏感信息

方案：

默认：

显示：

```
GPT-5
剩余额度:
80%
```

不要显示：

```
API Key
账号邮箱
Token
```

------

# 9. 自动更新安全

禁止：

下载：

```
未知exe
```

更新包：

必须：

- HTTPS
- SHA256校验
- 签名验证

------

# 10. 权限控制

启动时：

不要默认申请：

- 管理员权限
- 文件系统权限
- 摄像头
- 麦克风

遵循：

最小权限原则。

------

# 11. 防止恶意插件

如果支持插件：

必须：

插件沙箱。

禁止：

插件访问：

```
CredentialManager
API Key
用户文件
```

------

# 12. 安全测试项目

增加测试：

| 编号    | 测试                   |
| ------- | ---------------------- |
| SEC-001 | API Key不会明文保存    |
| SEC-002 | 日志不会输出Key        |
| SEC-003 | Cookie不会被读取       |
| SEC-004 | Codex登录不会泄露Token |
| SEC-005 | 图片不会上传           |
| SEC-006 | GIF大小限制            |
| SEC-007 | 恶意文件拒绝           |
| SEC-008 | HTTPS验证              |
| SEC-009 | 代理环境安全           |
| SEC-010 | 数据库加密             |
| SEC-011 | 崩溃日志脱敏           |
| SEC-012 | 配置文件权限           |
| SEC-013 | 悬浮窗隐藏敏感信息     |
| SEC-014 | 更新包验证             |
| SEC-015 | 异常退出恢复           |
