# 安全修改记录

| 模块 | 修改摘要 | 编译/测试记录 |
|---|---|---|
| 安全基础层 | 新增 `SensitiveDataFilter`、安全 HTTP Client、AES-256-GCM、零化 | Rust 初次测试 50/51，修正测试自扫描后通过 |
| Codex | 删除认证文件/Token 读取与 Bearer 请求，改为公开 CLI 状态 | Rust 测试通过 |
| 图片/GIF | 应用目录隔离、内部协议、格式/尺寸/帧数/SVG 校验、旧 Data URL 迁移 | TypeScript 检查通过；Rust asset tests 通过 |
| Key/数据库 | Keyring 主密钥、Key 掩码、`secure_settings`、敏感字段迁移、隐私默认关闭 | 前端生产构建通过；Rust 60 项阶段测试通过 |
| 网络/Provider | 内置 endpoint Registry、HTTPS-only、TLS、无重定向/代理、无响应正文 | URL/HTTP/客户端安全测试通过 |
| CSP/权限/悬浮窗 | 自定义资源 CSP、移除 opener、悬浮窗秘密字段回归门禁 | 生产前端构建与 SEC 静态测试通过 |
| 自动更新 | 公钥/HTTPS endpoint 守卫，未配置时 fail-closed | updater security test 通过 |
| Windows/Unix 文件权限 | 共享私有路径加固；Windows 受保护 DACL 仅允许 SYSTEM/对象所有者，Unix 0700/0600 | Windows 临时目录与文件真实 ACL 测试通过 |
| 自定义 Provider | 首次 origin 授权、UI 风险确认、IP/内网域名拒绝、DNS 私网检查与解析固定 | 授权作用域、私网分类及 URL 回归测试通过 |
| 更新防降级 | 严格语义版本比较、确认版本绑定、阻止降级与重复安装 | 升级/降级/版本替换测试通过 |
| 依赖审计 | 新增前端生产依赖审计、RustSec 命令及 CI 阻断任务 | npm 审计可本地执行；RustSec 等待 CI/工具环境 |
| 最终验证 | 格式、Clippy、前后端测试、依赖审计、日志扫描 | 结果记录于 `Security_Test_Report.md` 和最终交付说明 |
