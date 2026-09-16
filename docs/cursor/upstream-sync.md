# Cursor 接入与上游同步策略

生成日期：2026-09-05<br>
适用范围：`cc2cx` 主仓，以及需要持续跟随 `cc-switch` 上游变更的 Cursor 接入代码。

## 1. 同步原则

`cc2cx` 是产品主仓。上游同步只更新产品壳、通用配置、Provider 和发布基础设施，不把上游实现直接覆盖到 Cursor 接入边界。

Cursor 代码集中在 `src-tauri/src/cursor/`、`src-tauri/src/commands/cursor.rs`、Cursor 前端组件、Cursor 测试和 `docs/cursor/`。这些目录是独立所有权边界，任何上游同步都必须保留其行为和测试。

同步优先级如下：

1. 先同步上游高频通用文件的最新版本，并记录冲突。
2. 再人工恢复 Cursor 的注册点、状态注入和启动/退出清理。
3. 最后运行 Cursor 专项回归和现有产品回归，不能只依赖上游原有测试。

## 2. 文件所有权

| 文件或目录 | 同步策略 | 冲突处理 |
|---|---|---|
| `src-tauri/src/cursor/**` | 以本仓 Cursor 实现为准 | 只有明确的协议/平台修复才人工移植上游思想，不整文件覆盖 |
| `src-tauri/src/commands/cursor.rs` | 以本仓命令契约为准 | 保留命令名、错误字符串和状态结构 |
| `src/components/settings/CursorIntegrationPanel.tsx` | 以本仓 Cursor UI 为准 | 保留状态轮询、CA 操作和禁用条件 |
| `src-tauri/tests/cursor_*.rs` | 以本仓测试为准 | 上游行为变化必须先增加或修改回归测试 |
| `src-tauri/src/lib.rs` | 上游主体优先 | 人工恢复 `pub mod cursor`、命令注册、启动恢复和退出清理注册点 |
| `src-tauri/src/store.rs` | 上游主体优先 | 人工恢复 `AppState.cursor_harness` 与构造逻辑 |
| `src-tauri/src/config.rs` | 上游主体优先 | 保留私有原子写入和 Windows ACL 逻辑，确认通用写入行为未回退 |
| `src-tauri/Cargo.toml` | 合并上游依赖后人工审阅 | 保留 Cursor 依赖及目标平台 feature，禁止无审阅的版本升级 |
| `src-tauri/Cargo.lock` | 与清单一起生成 | 只接受可解释的 Cursor 依赖闭包和必要版本变更 |
| `src/i18n/locales/*` | 上游主体优先 | 保留 `settings.advanced.cursor` 完整键集，并运行 locale coverage |
| `docs/cursor/**` | 以本仓事实为准 | 同步后更新 commit、验证结果和未实现边界 |

## 3. 每次同步流程

### 3.1 同步前

记录以下信息：

- 上游仓库、分支和 commit。
- 本仓当前 commit、工作树状态和 Cursor 专项测试基线。
- 当前 `Cargo.toml`/`Cargo.lock` 是否一致。
- 正在进行的真实 Cursor 验收或协议 fixture 变更。

同步前必须保证工作树可回滚。不得在未记录基线的情况下直接执行大范围 rebase、自动格式化或锁文件全量重生成。

### 3.2 合并后

按以下顺序恢复：

1. 检查 `src-tauri/src/lib.rs` 的模块导出、Tauri 命令注册、启动恢复和退出清理。
2. 检查 `src-tauri/src/store.rs` 是否仍构造 `CursorHarness`。
3. 检查 `src-tauri/src/config.rs` 的 `atomic_write_private` 和 Windows 私有 ACL。
4. 检查前端 i18n 键集和 Cursor 面板是否仍挂在代理设置页。
5. 检查 `Cargo.toml` 与 `Cargo.lock`；先执行 `cargo metadata --locked`，再执行编译。
6. 检查 Cursor 路由白名单、CA 安装策略、settings 外部修改保护和停止恢复。

### 3.3 必跑验证

在具备完整 Rust/Windows 工具链时执行：

```powershell
pnpm typecheck
pnpm exec prettier --check src/components/settings/CursorIntegrationPanel.tsx src/components/settings/ProxyTabContent.tsx src/lib/api/settings.ts
pnpm exec vitest run
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --no-deps
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_ca
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_commands
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_routes
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_settings
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_protocol
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_adapter
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_backend
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_provider
cargo test --manifest-path src-tauri/Cargo.toml --test cursor_transport
git diff --check
```

若缺少 `link.exe`、Windows SDK 或平台证书工具链，必须把结果标为“环境阻断”，不能标为通过。

## 4. Cursor 专项回归门

同步后只有同时满足以下条件，才允许合并到发布分支：

- Windows CA 生成、精确检测、独立显式安装和停止后状态符合当前授权契约。
- settings 原值可恢复；外部修改不会被覆盖。
- 代理停止、异常退出和应用重启不会留下失效代理设置。
- 非 Cursor 域名不被 MITM；未列入白名单的 Cursor 路径按策略透传或拒绝。
- `Cargo.toml` 与 `Cargo.lock` 可用 `--locked` 解析。
- 真实 Cursor 通过自定义 endpoint/key 的普通对话、流式响应和取消场景均有记录；不要求官方账号登录，未验收项不得写成“已支持”。

当前 P2-0/P2-1 的 `BidiAppend`/`RunSSE` 首片 wire subset、合成流和断开传播已实现，P2-2 的通用可取消 Provider bridge、注入式 OpenAI-compatible Chat SSE source 和严格 factory，以及 P2-3a 的只读 current-provider 绑定、P2-3b 的 presence-aware 能力检测和未实现字段前置拒绝、P2-3c 的无请求体启动前 preflight/失败回滚已落地。2026-09-07 低并发专项回归中，P2 五个测试文件共 92/92 通过（protocol 18、adapter 15、backend 17、provider 31、transport 11），commands 10/10；database-backed Harness/MITM backend 生命周期、启动探测失败阻断、失败回滚、Provider 观测错误健康联动、脱敏 `/health=503` 和 backend header allowlist 已接入。故障转移绑定、完整 Agent runtime、真实 Cursor 流式响应/取消、动态选路和 H2 性能仍未实现或验收。fixture、日志和提交不得包含真实 token、Cookie、用户提示词、文件内容、图片、API key 或 CA 私钥，即使本地验收获得了明确授权。合成 backend 只负责验证协议、Provider 和 transport 边界，不能被当作完整 Agent runtime 或真实 Cursor 支持。上游同步不能改变这一结论。

## 5. 冲突记录格式

每次同步在变更说明中至少记录：

```text
upstream: <repo>@<commit>
cursor-boundary: preserved | repaired | intentionally changed
lockfile: unchanged | dependency-closure | requires-review
tests: <commands and result>
real-cursor-e2e: passed | blocked | not-run
unresolved: <items>
```

任何未解决冲突都必须指向具体文件和行为，不得只写“已解决冲突”。
