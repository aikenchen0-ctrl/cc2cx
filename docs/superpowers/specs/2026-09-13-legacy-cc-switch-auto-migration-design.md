# 旧 CC Switch 一键迁移设计

## 目标

用户在安装 Agent 页面点击一次即可从旧 CC Switch 数据库迁移配置；不要求用户先手动导出 SQL。手动 SQL 导入保留为自动快照失败时的降级入口。

## 数据流

`~/.cc-switch/cc-switch.db` → 只读 SQLite Backup 快照 → 内存数据库 → 现有 SQL 导出/校验与 schema migration → 当前 cc2cx 数据库安全备份 → 事务替换 → post-import 同步。

后端只使用固定旧数据目录，不接受前端任意数据库路径，避免把一键迁移变成任意文件读取接口。旧数据库不写入、不删除；迁移期间使用现有数据库恢复锁串行化。

## 行为与失败处理

- 旧数据库不存在：返回明确状态，前端继续显示手动 SQL 导入。
- 旧数据库无法只读打开、快照失败、schema 不兼容或导入失败：当前数据库保持不变，前端提示失败原因并保留手动 SQL 按钮。
- 导入成功：复用现有 `post_sync`，刷新 Agent/供应商状态。
- 现有本地运行日志等表按 `SYNC_PRESERVE_TABLES` 保留；旧 CC Switch 的配置表作为迁移源。
- 不输出、解析或新增任何 API Key、Cookie 或用户提示词日志。

## 验收

- 旧 DB 存在时，点击“一键同步旧配置”不打开文件选择器。
- 自动迁移成功后显示成功消息，旧 DB 仍存在，当前库产生安全备份。
- 自动迁移失败时不污染当前库，手动 SQL 导入仍可用。

## 实现状态

- 已增加固定旧数据库路径解析与只读 SQLite Backup 快照。
- 已复用现有 staging、schema 校验、迁移、安全备份和恢复锁。
- Agent 安装页检测到旧 DB 时直接调用自动迁移；手动 SQL 保留为降级入口。
- 已通过自动迁移 Rust/前端专项测试；当前 Debug renderer 与 Rust 二进制均已重建。

## 真实旧数据库预检（2026-09-13）

- 旧库 `~/.cc-switch/cc-switch.db` 只读完整性检查返回 `ok`。
- 旧库 `user_version=16`，当前导入器支持的 schema 版本为 18，属于可迁移范围。
- 已确认 `providers`、`provider_endpoints`、`mcp_servers`、`prompts`、`skills`、`skill_repos`、`settings` 均存在。
- 已在本机执行一次真实自动迁移：旧库保持不变，当前库完整性检查为 `ok`、版本 17，并生成新的安全备份；后续重复迁移仍应由用户主动点击“一键同步旧配置”触发。

## Schema 18 兼容更新（2026-09-17）

- CC Switch v3.20.1+ 将 `user_version` 提升到 18；v17 → v18 只为
  `session_log_sync` 增加可空的 `last_byte_offset` 与
  `last_tail_fingerprint` 列。
- cc2cx 现在会幂等补齐这两列并将数据库版本迁移到 18；已有行保持 `NULL`，不删除配置或会话数据。
- 本次只移植 schema 18 的兼容迁移；上游 schema 19 的 `enabled_mcode` 变更需单独评估，不纳入本修复。
