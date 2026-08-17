# AI API Monitor P0 数据恢复与迁移安全手动验收清单



> 仅使用测试账户、测试 Provider、测试 API Key 和测试数据库。

>

> 所有破坏性测试开始前必须完全退出 AI API Monitor，并备份整个应用数据目录。



---



## 0. 基线与环境准备



### P0-PRE-01 基础质量门禁



- [x] 进入项目目录：



```powershell

cd "E:\Project\AI API Monitor"

```



- [x] 如需验证干净环境，优先执行：



```powershell

pnpm install --frozen-lockfile

pnpm check

pnpm build

```



- [x] `pnpm check` 通过。

- [x] `pnpm build` 通过。

- [x] 记录当前 Git Commit。

- [x] 记录当前应用版本。



---



### P0-PRE-02 确认真实应用数据目录



不要直接假设数据目录一定是：



```text

%LOCALAPPDATA%\com.aiapimonitor.desktop

```



应优先通过应用日志、Tauri 配置或实际运行结果确认。



确认后：



```powershell

$data = "$env:APPDATA\com.aiapimonitor.desktop"



Test-Path $data

Get-ChildItem $data -Force

```



- [x] 数据目录确认无误。

- [x] 确认当前目录属于测试环境。

- [x] 找到实际数据库 `ai-api-monitor.db`。

- [x] 确认日志目录。

- [x] 确认 `migration-snapshots` / recovery 文件所在位置。



---



### P0-PRE-03 完整备份



必须在应用完全退出后执行：



```powershell

$backup = "$env:USERPROFILE\Desktop\AI-API-Monitor-test-backup"



Remove-Item $backup -Recurse -Force -ErrorAction SilentlyContinue

Copy-Item $data $backup -Recurse -Force

```



确认：



- [x] `ai-api-monitor.db` 已备份。

- [x] `ai-api-monitor.db-wal`（如存在）已备份。

- [x] `ai-api-monitor.db-shm`（如存在）已备份。

- [x] 配置文件已备份。

- [x] migration snapshots 已备份。（20260817-170854    N/A：当前测试环境尚未生成 migration-snapshots）

- [x] 测试凭据可以重新创建。



禁止针对真实生产数据执行破坏性测试。



---



# 1. 数据库损坏恢复



## P0-DB-01 主数据库损坏



### 操作



完全退出应用。



```powershell

$db = Join-Path $data "ai-api-monitor.db"



Copy-Item $db "$db.before-corrupt" -Force

Set-Content $db "invalid sqlite test data"

```



然后启动安装版 AI API Monitor。



### 验收



- [x] 应用不会直接崩溃退出。
- [x] 应用不会无限启动循环。
- [ ] 损坏数据库不会被静默覆盖。
- [ ] 原损坏数据库被保存到 recovery 文件。
- [ ] recovery 文件包含时间戳。
- [ ] recovery 文件包含随机标识或其他避免重名机制。
- [ ] 创建新的安全数据库。
- [ ] 应用可以进入可恢复/可使用状态。
- [ ] 用户能够看到明确的数据库恢复提示。
- [ ] 日志存在 `database_recovery` 或等效事件。logs 目录存在，但数据库损坏启动失败期间没有生成任何应用日志。
- [ ] 日志说明恢复原因。
- [ ] 日志不包含完整 API Key / Token / Cookie。
- [ ] recovery / migration snapshot 中不存在明文 API Key。

P0-DB-01：FAIL

附加问题：
数据库损坏导致启动退出时，应用 logs 目录未生成任何诊断日志。
无法通过应用日志定位 database_recovery / SQLite failure。

实际行为：
1. 数据库损坏后应用直接退出。
2. 无法进入主界面。
3. 无安全恢复界面。
4. 无恢复提示。
5. logs 目录存在，但没有生成日志文件。
6. 重复启动仍无法恢复。

附加验收缺陷：
启动早期数据库故障缺少持久化诊断日志。



### 恢复



关闭应用。



恢复数据库前，同时删除当前 SQLite WAL/SHM：



```powershell

Remove-Item "$db-wal" -Force -ErrorAction SilentlyContinue

Remove-Item "$db-shm" -Force -ErrorAction SilentlyContinue

Remove-Item $db -Force -ErrorAction SilentlyContinue



Move-Item "$db.before-corrupt" $db

```



- [ ] 原测试数据库可以重新正常启动。



---



# 2. 数据库锁冲突



数据库锁测试和应用多实例测试应分开执行。



## P0-DB-02 多实例行为



### 操作



- [ ] 启动实例 A。

- [ ] 保持实例 A 正常运行。

- [ ] 再启动实例 B。



### 验收



如果产品设计禁止多实例：



- [ ] B 不会启动第二套数据库连接。

- [ ] B 有明确提示或自动聚焦 A。

- [ ] 不产生数据库损坏。

- [ ] 不创建 recovery 数据库。



如果产品允许多实例：



- [ ] 两实例均正常运行。

- [ ] SQLite 并发机制工作正常。

- [ ] 不发生数据覆盖。



---



## P0-DB-03 SQLite 写锁冲突



仅仅“打开 SQLite GUI”不一定会产生有效数据库锁。



必须使用能够保持写事务的测试程序、SQLite CLI 或故障注入，让数据库保持真实的写锁。



例如保持：



```sql

BEGIN IMMEDIATE;

```



或：



```sql

BEGIN EXCLUSIVE;

```



且不要提交事务。



### 验收



短时间锁定：



- [ ] 应用自动重试。

- [ ] 不立即判断数据库损坏。

- [ ] 不触发 `database_recovery`。

- [ ] 不创建新的空数据库。

- [ ] 不覆盖原数据库。

- [ ] 日志记录锁冲突和重试。



长时间锁定：



- [ ] 最终停止无限重试。

- [ ] 向用户显示数据库正在被占用。

- [ ] 错误信息可理解。

- [ ] 原数据库保持完整。

- [ ] 不执行损坏恢复流程。



解除锁之后：



- [ ] 应用无需人工修数据库即可重新正常运行。



---



# 3. 迁移前快照



## P0-MIG-01 低版本数据库迁移



### 准备



准备一个真实的旧 Schema 测试数据库。



记录迁移前：



- [ ] Schema Version。

- [ ] Provider 数量。

- [ ] Provider ID。

- [ ] Provider 名称。

- [ ] Provider 类型。

- [ ] Provider URL。

- [ ] 历史数据数量。



### 操作



将旧数据库放入测试数据目录，然后启动应用。



### 验收



- [ ] 数据库升级前先创建 snapshot。

- [ ] snapshot 创建成功后才执行破坏性 Schema 迁移。

- [ ] snapshot 文件不会覆盖旧 snapshot。

- [ ] snapshot 文件名可追踪版本。

- [ ] snapshot 文件名包含时间戳。

- [ ] snapshot 文件名包含唯一标识。



推荐格式：



```text

ai-api-monitor-v<source-version>-<timestamp>-<random-id>.db

```



或项目实际定义的等价格式。



---



## P0-MIG-02 快照唯一性与轮转



注意：完成一次迁移之后数据库已经是最新版。



因此不能简单地连续重启应用测试多次迁移。



每次测试前必须重新恢复同一份旧版本数据库。



执行：



```text

旧数据库

→ 启动

→ snapshot 1

→ 恢复旧数据库

→ 启动

→ snapshot 2

→ ...

```



验收：



- [ ] 多次 snapshot 文件名不同。

- [ ] 后一次 snapshot 不覆盖前一次。

- [ ] 所有 snapshot 都可以正常打开。

- [ ] snapshot Schema 与迁移前数据库一致。



如果项目设计明确规定：



```text

最多保留 5 份 snapshot

```



则继续验证：



- [ ] 第 6 次 snapshot 创建成功。

- [ ] 只删除最旧 snapshot。

- [ ] 最近 5 份得到保留。

- [ ] 当前数据库绝不会被轮转逻辑删除。



如果“保留 5 份”尚未写入产品要求，则不要把 `5` 当成硬性验收标准。



---



# 4. 旧凭据 UUID 迁移



## P0-CRED-01 Legacy Credential → UUID



### 准备



建立测试 Provider。



旧引用示例：



```text

com.aiapimonitor.desktop:provider_test

```



Windows Credential Manager 中创建对应测试凭据。



必须使用专用测试 Key，例如：



```text

sk-p0-migration-canary-<随机UUID>

```



禁止使用真实 API Key。



### 验收



首次启动：



- [ ] Provider 正常显示。

- [ ] Provider ID 不变化。

- [ ] Provider 名称不变化。

- [ ] Provider 类型不变化。

- [ ] Provider URL 不变化。

- [ ] Provider 历史数据不变化。



迁移后：



```text

com.aiapimonitor.desktop:key_<uuid>

```



- [ ] `key_ref` 已更新为 UUID 格式。

- [ ] 新 Credential 已成功创建。

- [ ] 新 Credential 内容与原测试 Credential 一致。

- [ ] 数据库没有保存 Credential 明文。



重新启动应用：



- [ ] UUID 不再次改变。

- [ ] 不生成第二个 Credential。

- [ ] 不产生重复 Provider。

- [ ] 不重复执行迁移。



旧 Credential 清理：



- [ ] 新 Credential 可用之后才删除旧 Credential。

- [ ] 旧 Credential 删除失败不会破坏新 Credential。

- [ ] 删除失败有明确日志。

- [ ] 日志不包含 Credential 明文。

- [ ] 下次启动不会因此重新生成 UUID。



---



# 5. 凭据删除补偿



## P0-CRED-02 Credential Manager 删除失败



优先使用 Debug/Test Fault Injection。



不要依赖随机破坏 Windows Credential Manager 权限来制造故障，因为结果不可重复。



### 操作



- [ ] 创建测试 Provider。

- [ ] 确认 DB Provider 存在。

- [ ] 确认 Windows Credential 存在。

- [ ] 开启 `credential_delete_fail` 类故障注入。

- [ ] 在 UI 中删除 Provider。



### 验收



- [ ] UI 删除操作返回失败。

- [ ] 不显示“删除成功”。

- [ ] DB Provider 仍存在。

- [ ] Credential 仍存在。

- [ ] 日志明确记录 Credential 删除失败。

- [ ] 日志没有测试 Key 明文。



关闭故障注入：



- [ ] 再次删除能够成功。

- [ ] DB Provider 删除。

- [ ] Credential 删除。



---



## P0-CRED-03 数据库删除失败补偿



### 操作



- [ ] 创建测试 Provider。

- [ ] 记录 Credential 的测试值。

- [ ] 确认 Credential 存在。

- [ ] 使用数据库故障注入或真实 DB 写锁。

- [ ] 删除 Provider。



### 验收



如果系统 Credential 已删除、DB 删除随后失败：



- [ ] 补偿逻辑被执行。

- [ ] Credential 被重新创建。

- [ ] 恢复后的 Credential 内容与删除前一致。

- [ ] DB Provider 记录仍存在。

- [ ] Provider 重新刷新仍可使用。

- [ ] 日志包含 `rollback`、`compensation` 或等价事件。

- [ ] 日志不包含 Credential 明文。



解除数据库故障：



- [ ] 再次删除可以正常完成。



同时测试极端情况：



```text

DB 删除失败

+

Credential 恢复也失败

```



要求：



- [ ] 不报告“删除成功”。

- [ ] DB Provider 不被静默删除。

- [ ] 日志明确记录 compensation failure。

- [ ] 用户能够知道该 Provider 需要恢复处理。



---



# 6. 强制退出与 WAL 恢复



## P0-DB-04 Crash / WAL Recovery



### 操作



- [ ] 创建测试 Provider。

- [ ] 至少完成一次成功刷新。

- [ ] 记录已经确认提交的历史记录数量。

- [ ] 再执行刷新/保存。

- [ ] 在写入期间使用任务管理器强制结束进程。

- [ ] 重新启动应用。



### 验收



- [ ] 应用正常启动。

- [ ] 不错误触发数据库损坏恢复。

- [ ] 已经提交成功的 Provider 数据仍存在。

- [ ] 已经提交成功的历史数据仍存在。

- [ ] 未完成事务允许被 SQLite 回滚。

- [ ] 不出现半条 Provider 数据。

- [ ] 不产生重复 Provider。

- [ ] 不产生重复 Schema Migration。

- [ ] SQLite integrity check 正常。

- [ ] 日志不包含敏感信息。



注意：



“WAL 文件重新启动后仍然存在”不是验收条件。



SQLite 可以自动 checkpoint 或删除 WAL。



真正要验证的是：



```text

事务一致性 + 已提交数据恢复

```



---



# 7. 敏感数据泄漏扫描



## P0-SEC-01 Canary Secret 扫描



不要只搜索：



```text

secret

password

Authorization

```



因为这些字段名称本身可能合法存在，会产生大量误报。



在整个 P0 测试开始前生成唯一测试凭据，例如：



```powershell

$canary = "sk-p0-canary-$([guid]::NewGuid().ToString())"

$canary

```



后续所有 Credential 测试使用这个专用值。



测试结束后首先搜索这个精确 Canary：



```powershell

Get-ChildItem $data -Recurse -File |

    Select-String -SimpleMatch $canary -ErrorAction SilentlyContinue

```



### 必须检查



- [ ] 应用日志。

- [ ] crash 日志。

- [ ] recovery metadata。

- [ ] migration metadata。

- [ ] 配置文件。

- [ ] 导出文件。

- [ ] 临时文件。



随后进行通用扫描：



```powershell

Get-ChildItem $data -Recurse -File |

  Select-String `

    -Pattern "Bearer\s+[A-Za-z0-9._\-]+|Authorization\s*:|Cookie\s*:|access_token|refresh_token" `

    -ErrorAction SilentlyContinue

```



### 不允许存在



- [ ] 完整 API Key。

- [ ] 完整 Access Token。

- [ ] Refresh Token。

- [ ] Session Cookie。

- [ ] Authorization Header 内容。

- [ ] Windows Credential 密码。



允许存在：



- [ ] 字段名称。

- [ ] 测试说明。

- [ ] 已脱敏值。

- [ ] `sk-****1234` 类 UI 脱敏结果。



注意：



SQLite `.db` 属于二进制文件，PowerShell `Select-String` 不能作为唯一的数据库敏感数据验证手段。



应额外通过 SQLite 查询确认：



- [ ] Provider 表不存在 API Key 明文字段值。

- [ ] `key_ref` 只保存引用。

- [ ] Migration Snapshot 中同样不存在凭据明文。



---



# 8. 最终回归



## P0-REG-01 功能回归



完成全部故障测试并恢复干净测试环境。



- [ ] 正常启动应用。

- [ ] 创建 Provider。

- [ ] 编辑 Provider。

- [ ] 刷新 Provider。

- [ ] 查看历史记录。

- [ ] 重启 Provider 仍存在。

- [ ] 删除 Provider。

- [ ] 删除后 Credential 同步消失。

- [ ] 应用正常退出。

- [ ] 再次启动正常。



---



## P0-REG-02 最终质量门禁



```powershell

pnpm check

pnpm build

pnpm security:audit

```



如果要求验证完全可复现的依赖安装：



```powershell

pnpm install --frozen-lockfile

pnpm check

pnpm build

pnpm security:audit

```



- [ ] `pnpm check` 通过。

- [ ] `pnpm build` 通过。

- [ ] `pnpm security:audit` 达到项目定义的安全门禁。

- [ ] 没有因 P0 修复引入新的严重安全问题。



---



# 9. 每项测试记录



```text

测试编号：

测试名称：



测试日期：

Windows 版本：

AI API Monitor 版本：

Git Commit：



测试数据目录：

数据库版本：



前置条件：



执行步骤：



预期结果：



实际结果：



相关日志：



Recovery / Snapshot 文件：



是否通过：

PASS / FAIL / BLOCKED



失败原因：



截图 / 日志路径：



备注：

```



---



# 10. 推荐执行顺序



- [ ] 0\. 完整备份测试环境

- [ ] 1\. 数据库损坏恢复

- [ ] 2\. 多实例行为

- [ ] 3\. SQLite 锁冲突

- [ ] 4\. 迁移前快照

- [ ] 5\. Snapshot 唯一性与轮转

- [ ] 6\. Legacy Credential UUID 迁移

- [ ] 7\. Credential 删除失败

- [ ] 8\. DB 删除失败补偿

- [ ] 9\. Crash / WAL 恢复

- [ ] 10\. Canary 敏感数据扫描

- [ ] 11\. 正常功能回归

- [ ] 12\. `pnpm security:audit`



---



# P0 最终通过条件



只有以下条件全部满足，P0 数据恢复任务才能判定完成：



- [ ] 数据库损坏不会导致不可恢复启动失败。

- [ ] DB Lock 不会被误判为 DB Corruption。

- [ ] 数据库恢复不会覆盖原始损坏数据。

- [ ] Schema Migration 前一定存在可恢复 Snapshot。

- [ ] Snapshot 不覆盖且轮转逻辑安全。

- [ ] Legacy Credential 只迁移一次。

- [ ] Credential UUID 在重启后保持稳定。

- [ ] Credential 删除与 DB 删除具备失败补偿。

- [ ] Crash/WAL 场景保持事务一致性。

- [ ] DB、Snapshot、Recovery、日志中没有 Credential 明文。

- [ ] 最终正常 Provider CRUD 和刷新功能没有回归。

- [ ] `check / build / security:audit` 最终通过。