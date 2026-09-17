# CC Switch 上游能力迁移路线

更新日期：2026-09-17

## 目标

在不降低 cc2cx 二开能力的前提下，逐项吸收 `farion1231/cc-switch` 的稳定性、兼容性、性能和供应商能力。当前 schema 18 兼容已作为独立热修复发布；本文件记录其余能力的实际差异和迁移顺序。

## 不可回退的二开边界

- `src-tauri/src/cursor/**`、Cursor Tauri 命令/UI、Cursor 专项测试：以 cc2cx 为准。
- `session_transfer`、CASR 17 provider 写回/启动、`vendor/casr`：以 cc2cx 为准。
- Agent 安装、DeepSeek Harness、旧 CC Switch 一键迁移：以 cc2cx 为准。
- 现有 `proxy/`、Provider live 锁、Pi revision/会话语义：只允许函数级移植，禁止整文件覆盖。
- 每一波必须通过上游目标测试、cc2cx 全套回归和 Cursor/CASR/安装迁移专项回归后才可发布。

## 迁移波次

| 波次 | 能力 | 上游依据 | cc2cx 当前状态 | 处理方式与验收 |
|---|---|---|---|---|
| 0 | schema 18、会话同步列 | v3.20.1，`bcee61be` | 已完成，v3.20.7 | v17→v18 幂等迁移；legacy snapshot、备份、恢复测试 |
| 1 | Codex 0.149 config-only、legacy reroute、安全闸、Team 账号隔离 | `cbb79127`、`c2ec78dd`、`6243e20a`、`92d52916`、`15884b20` | 未完整同步 | 函数级移植 `codex_config`/live/auth；覆盖多账号、重登、删除、takeover、Cursor 路由；不覆盖现有 marker/lock |
| 2 | Claude 字节游标、截断/重写检测、自动/手动扫描、prefix cache | `bcee61be`、`f8d97348`、`f05e2033`、`092ea1f3`、`d8065cc6` | 仅有 schema 列，扫描仍按行号 | 先移植游标/指纹/事务，再接 UI；覆盖追加、半行、截断、同尺寸重写、读错和 Pi 保留语义 |
| 3 | Grok native Responses、OAuth、GPT-6、并行工具、Images | `b7da894b`、`914c8bb5`、`0a9a4378`、`9e110053`、`db346128`、`872ec775`、`17be9092` | 基础 sanitizer 存在，能力不完整 | 只改 xAI/Codex OAuth 分支；保留 Cursor handler/forwarder；覆盖 schema collapse、浮点参数、agent_message、SSE、图片端点 |
| 4 | DeepSeek MCP/catalog、Kimi/Moonshot Responses、智谱 GLM Responses | `5a040348`、`08984ba5`、`db41d701`、`e47b5fca` | 仍有旧 Chat 字段和旧 endpoint | 按 provider/endpoint 局部迁移；保留 Chat 转换和 Cursor 路由；catalog/preset/模型列表回归 |
| 5 | OpenCode Go 用量、Fable/新模型定价、新供应商预设 | `270a4ff3`、`460aa8c7`、`ccc140a2`、`68d71cc6`、`6d25f34e`、`b45b2bd1`、`389dd96c` | 后端/定价/多项 preset 缺失 | 增量 seed 不覆盖用户价格；逐条核对 endpoint、metadata、i18n、图标和 usage 测试 |
| 6 | Otty、无障碍、Prompt restore、更新错误 | `bd15ea11`、`c58a25b2`、`c911c7e3`、`bd1265d2` | 大多缺失或行为相反 | 局部移植；保持现有终端/CASR 启动器和用户手写 prompt；UI/i18n 回归 |
| 7 | MiniMax Code 与 schema 19 | upstream main `06082e18` | 完全缺失 | 独立 feature；新增 AppType/config/MCP/session/usage/UI 后再做 v18→v19，不能混入前六波 |

## 当前发布状态

- `v3.20.7` 已支持 CC Switch `user_version=18`，不会再因 schema 18 阻断旧库一键迁移。
- `v3.20.7` 的 Windows 安装包和 macOS arm64/x64 包已发布；均为未签名构建。
- 上游 main 当前为 schema 19，但 `enabled_mcode` 及 MiniMax Code 尚未迁移，不能宣称已跟随最新版。

## 每波固定质量门

1. 锁定上游 commit、文件和行为差异，先写失败测试。
2. 只移植必要函数/数据条目，保留 cc2cx 所有权边界。
3. 运行目标专项测试、`pnpm typecheck`、前端单测、Rust 全库测试、Clippy、格式和 `cargo metadata --locked`。
4. 运行 Cursor CA/透明入口、CASR 17 provider 写回、Agent 安装/旧库迁移回归。
5. 检查无真实凭据、请求体、用户文件和 CA 私钥进入 fixture、日志、文档或提交。
6. 记录未完成能力和真实环境阻断，不把“编译通过”写成“功能已支持”。
