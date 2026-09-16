# CASR 跨工具会话写回设计

## 目标

在 `cc2cx` 会话管理 UI 中实现跨工具会话互转：用户选择本地源会话、目标 provider 和工作目录后，由 CASR 读取源格式、转换为 canonical IR、写入目标工具的原生存储、读回校验，并提供可执行的恢复/打开入口。

## 当前实现状态（2026-09-16）

本设计已经落地到当前工作树：CASR 提供 17 个默认 provider 的能力查询、统一发现、显式源路径转换、canonical IR 校验、原子写回、读回校验和回滚；cc2cx 提供适配层、三个 Tauri command、CASR 与旧扫描器结果合并，以及会话管理中的写回对话框。

Hermes 仍由 cc2cx 旧扫描器负责，不属于 CASR 默认 17-provider 注册表。Hermes 会话可以继续通过旧路径列出和读取，但写回对话框会将 Hermes 来源标记为不支持，而不是伪造 CASR provider。Kiro 启动时先尝试 `kiro-cli`，找不到时回退到 `kiro`。

当前实现尚未提交；本文中的提交动作和发布动作不应被理解为已经完成。

主路径必须满足：

1. 不要求用户手抄 Session ID。
2. 不以纯命令行转换作为产品主路径。
3. 目标文件写回成功但校验失败时自动回滚。
4. 明确显示有损转换、只读目标和条件可写目标。
5. cc2cx 适配层不主动把 API key、Cookie、会话正文、用户提示词或 CA 私钥写入日志、fixture、文档或提交；CASR provider 的 verbose/trace 日志仍可能包含本地路径、Session ID 和 detection evidence，不能把所有日志承诺为匿名。

## 范围与非目标

首期覆盖 CASR 默认注册表中的全部 17 个 provider：

`Claude Code`、`Codex`、`Gemini CLI`、`Antigravity CLI`、`Cursor`、`Cline`、`Aider`、`Amp`、`OpenCode`、`ChatGPT`、`ClawdBot`、`Vibe`、`Factory`、`OpenClaw`、`Pi-Agent`、`Kiro CLI`、`Grok Build`。

首期不承诺所有 provider 都能作为写入目标。CASR 当前已明确：Antigravity 是 read/resume-only；OpenCode 的 live 1.x/2.x 数据库按 schema 只读，其他 provider 可能因本地版本或索引状态产生条件性警告。UI 必须把这些能力差异前置展示。

OpenCode 的会话删除也遵循同一安全边界：会话管理只对 legacy `sessions/messages/files` 布局执行事务删除；1.x `session/message/part` 与 2.x `session_v2/session_message` 是事件日志投影，直接删除会留下不可重建的一致性风险，因此明确拒绝。

本设计不包含：

- 复制或迁移 provider 凭据；
- 把 CASR CLI 作为用户必须单独安装的依赖；
- 重写 cc2cx 现有会话扫描器的所有 provider 解析逻辑；首期只增加 CASR discovery 结果的补充接入。

## 融合方式

`vendor/casr` 作为 vendored Rust library，以 path dependency 接入 Tauri crate：

```toml
casr = { package = "cross_agent_session_resumer", path = "../vendor/casr", default-features = false }
```

只编译 CASR library 模块，不把 CASR 的独立 CLI 入口、`target` 或构建缓存嵌入桌面产物。CASR 保留 provider reader/writer、canonical model、验证、原子写回和回滚实现；cc2cx 增加薄适配层，负责产品协议和平台交互。

外层仓库现在将 `vendor/casr` 作为可复现的 vendored 源码；原始 `casr`
目录保留为上游独立 checkout，用于同步和审阅，不参与 Tauri 构建。发布时
只需提交 `vendor/casr`，不应把原始 checkout 作为嵌套 gitlink 加入外层。

这样形成单向边界：

```text
SessionManager UI
  -> Tauri transfer IPC
  -> cc2cx CASR adapter
  -> CASR ProviderRegistry + canonical IR
  -> target native writer + read-back verification
  -> structured result
  -> UI success/warning/recovery action
```

## CASR API 扩展

现有 CLI pipeline 依赖全局 `resolve_session`，不适合 UI 已经选出源路径的场景。CASR library 增加不破坏 CLI 的显式入口：

```rust
pub fn convert_source_path(
    &self,
    source_alias: &str,
    source_path: &Path,
    target_alias: &str,
    opts: ConvertOptions,
) -> anyhow::Result<ConversionResult>;
```

该入口执行以下固定顺序：

1. 用 alias 或 slug 查找源、目标 provider。
2. 校验源路径为文件或 provider 允许的数据库路径，并调用源 reader。
3. 应用 `workspace_override`，将 UI 选定目录写入 canonical session。
4. 调用 `validate_session`，错误立即停止，警告进入结果。
5. 执行已有上下文预算、工具配对修复和有损转换提示逻辑。
6. 调用目标 writer 的原子写回。
7. 读取第一目标产物进行 read-back 验证；失败时调用 CASR rollback。
8. 返回目标 session ID、产物路径、resume command、备份路径和警告。

为让 UI 在写入前展示真实能力，Provider trait 增加默认能力描述，不改变既有实现的必需方法：

```rust
pub enum WriteSupport {
    Supported,
    ReadOnly { reason: String },
    Conditional { reason: String },
}
```

静态只读 provider 返回 `ReadOnly`；OpenCode 等需要检查目标数据库 schema 的 provider 返回 `Conditional`，在实际转换时再次执行 writer 自己的最终校验。能力查询失败不能被解释为可写。

### 源会话发现

`ProviderRegistry` 增加只读的统一发现入口，复用每个 provider 已有的 `list_sessions()`；当 provider 没有专用列表实现时，按其 `session_roots()` 做有限深度扫描并调用 `read_session` 过滤不可解析文件。发现结果至少包含 provider slug、CLI alias、session ID、虚拟或真实源路径和已解析的 canonical session。数据库 provider 的路径采用 CASR 的虚拟路径约定（例如 `opencode.db/<session-id>`），不使用 cc2cx 现有的 `sqlite:` 展示字符串；适配层在接收旧会话条目时提供兼容解析。

cc2cx 保留当前 8-provider 扫描器作为兼容和快速路径，再把 CASR discovery 结果合并：

1. 以规范化绝对路径优先去重；虚拟数据库路径以 provider + 数据库文件 + session ID 去重。
2. 同一 provider/session ID 有多个条目时，优先 CASR 能读回且路径更具体的条目。
3. CASR 无法枚举或读取的 provider 仍出现在目标能力列表，但不伪造源会话；UI 显示“未发现可恢复会话”及 provider 返回的原因。
4. 发现 API 本身只返回可解析的会话元数据，不把消息正文或凭据放入 discovery 结果；cc2cx 适配层日志记录 provider、阶段和错误类型。CASR provider 的 verbose/trace 日志仍可能包含本地路径、Session ID 或 detection evidence，详见下方安全边界。

## cc2cx 后端适配层

在 `src-tauri/src/session_transfer.rs` 中定义产品 DTO 和映射：

```rust
pub struct SessionTransferRequest {
    pub source_provider_id: String,
    pub source_session_id: String,
    pub source_path: String,
    pub target_provider: String,
    pub workspace: Option<String>,
    pub force: bool,
    pub enrich: bool,
    pub max_context_tokens: usize,
    pub max_tool_output: usize,
    pub keep_reasoning: bool,
}
```

cc2cx provider ID 映射到 CASR alias/slug 必须集中维护，不能散落在 UI：

| cc2cx ID | CASR alias |
|---|---|
| `claude` | `cc` |
| `codex` | `cod` |
| `gemini` | `gmi` |
| `grokbuild` | `grk` |
| 其他已扫描 provider | CASR 对应 alias |

适配层负责：

- 拒绝空路径、相对路径和不存在的源路径；
- 将工作目录规范化为绝对路径，并要求它是目录；
- 只接受 provider/session/path 标识，不接受 renderer 任意 shell 命令；
- 把 CASR 错误转为稳定的 `code/message/details` IPC 错误 envelope；
- 将结果中的绝对路径、目标 ID、警告和有损标记返回 UI，过滤凭据和正文。

新增 Tauri commands：

```text
list_session_transfer_targets
transfer_session
launch_transferred_session
```

`list_session_transfer_targets` 返回全部 provider 的名称、alias、检测状态、写入能力以及独立的启动能力。启动能力分为 `supported`、`executableMissing`、`unsupported`，不能用 session store 的 `installed` 检测结果代替。`transfer_session` 执行上述 CASR pipeline。`launch_transferred_session` 只接受结构化目标 provider、session ID 和 workspace，由后端生成 argv/启动参数，不接受任意命令字符串。

## UI 工作流

在 `SessionManagerPage` 的详情操作区增加“转换并写回”入口和独立对话框：

1. 源会话由当前选中项自动带入，对话框显示标题和来源 provider；源路径与 Session ID 由后端从选中项传入，不提供任意命令编辑框。
2. 目标选择器展示 17 个 provider；未安装、只读和条件可写状态直接可见。
3. 默认工作目录为源会话 `projectDir`；用户可用已有 `pickDirectory` 选择器更换。
4. 目标为只读时禁用确认按钮并显示 CASR 原因；条件可写目标显示风险说明。
5. 确认前显示“可能丢失”的字段：隐藏推理、工具细节、模型元数据、目标格式不支持的附件等。
6. 执行期间锁定对话框并显示统一的“写回中”状态；读取、验证、写回和校验阶段不会逐阶段呈现在当前 UI 中。
7. 成功后显示目标 provider、目标 session ID、写入位置、备份位置和警告；只有 `launchSupport=supported` 时显示“打开新会话”，其余状态明确要求在目标工具中手动恢复。
8. 校验失败时后端自动回滚；当前 UI 显示错误消息，未提供专用 rollback 结果面板。若回滚失败，IPC 返回 `rollback_failed`，调用方可据此提示人工处理。

UI 不直接拼接或执行 resume command，也不要求用户复制命令。当前对话框默认使用 `force: false`；若目标已存在，会显示明确的“覆盖并重试”操作，只有用户主动点击后才使用 `force: true`。

## 恢复与平台行为

`launch_transferred_session` 使用 provider 启动器注册表：

- Windows：后端用受控 argv 在新的可见控制台中启动；npm `.cmd`/`.bat` shim 只接收后端生成的参数。
- macOS：后端将结构化 program/argv 逐项 POSIX 转义并再次执行 AppleScript 字符串转义，然后由 Terminal.app 打开可见会话；不接受 renderer 提供的命令文本。
- Linux：当前自动打开能力明确标记为 `unsupported`，写回结果保留并提示手动恢复，直至加入经过验证的可见终端适配器。
- Unix executable 探测要求普通文件具有执行位；找不到 executable 时保留已完成的写回并返回非致命 warning。
- Kiro 的 executable 解析顺序为 `kiro-cli`、`kiro`。
- provider 没有结构化恢复命令或 executable 缺失时，成功写回仍保留；能力列表会提前声明该状态，UI 不显示误导性的自动打开按钮。

## 错误、冲突与数据安全

- 默认不覆盖已有目标。IPC/CASR 支持 `force` 字段并保留 CASR `.bak`；目标冲突时 UI 提供显式覆盖重试，不会静默删除已有会话。
- 目标 provider 未安装时，可以在 writer 支持的情况下写入，但必须显示“目标 CLI 未检测到，恢复可能失败”。
- 源 reader 失败、目标能力为只读、schema 不匹配、读回不一致和回滚失败分别使用稳定错误码。
- cc2cx 适配层日志只记录 provider、阶段、耗时和错误类型，不主动记录消息正文、token、Cookie、API key、用户提示词或私钥；CASR provider 的 verbose/trace 日志仍可能记录本地路径、Session ID 和 detection evidence。
- fixture 使用合成 provider 文件和临时目录；真实本机验证产物不进入仓库。

## 测试与验收

### CASR library

- 指定源 provider/path 的读入和目标 alias 选择。
- workspace override 的绝对化和目标 writer 使用。
- 全部 17 provider 的能力矩阵；Antigravity 只读、OpenCode schema 条件分支。
- 原子写回、冲突、备份、read-back mismatch 和 rollback。
- 既有 CLI pipeline 回归，确保新增入口不改变 CLI 行为。

### cc2cx Tauri

- DTO 校验、provider ID 映射、错误 envelope 和敏感字段过滤。
- `transfer_session` 成功、只读拒绝、目标未安装警告、路径错误和回滚结果。
- Windows/macOS 启动器验证结构化 argv 和 cwd；macOS 的 shell 文本完全由后端逐参数转义生成。Linux 当前验证显式的手动恢复降级，不宣称已自动打开。

### React UI

- 目标列表状态渲染和只读/条件可写禁用逻辑。
- 目录选择、确认、进度、成功、警告、失败状态及后端 rollback 错误展示。
- 成功后刷新会话列表并可从 UI 直接触发恢复。

验收标准是：至少用合成 fixture 完成每个可写 provider 的一条跨 provider 转换并读回校验；只读 provider 必须在写入前被阻止；任一校验失败不得留下未经提示的半成品。
- 当前自动化证据分层记录：CASR 全量测试历史记录为 `1188/1188`，本轮 CASR library 为 `615/615`；本轮 cc2cx 的 `session_transfer` 为 18/18、provider matrix 为 2/2、启动相关 library 单元测试为 8/8；前端 transfer dialog 为 14 项、SessionManagerPage 为 16 项，最近定向运行是 `30/30`。provider matrix 只验证映射、写入/启动能力边界和结构化启动参数，不等同于 17 个 provider 都完成了真实 IPC `transfer_session`。
- 因此，“每个可写 provider 的真实写回”仍是验收待办，而不是当前已证明的产品事实；真实本机数据、凭据和会话正文不纳入 fixture 或测试日志。

## 后续同步策略

CASR 更新先在原始 `casr` checkout 中审阅和测试，再同步到 `vendor/casr`；适配层只依赖稳定的 library API 和 DTO，不复制 provider 实现。每次 CASR 更新必须运行 provider capability、round-trip 和 cc2cx IPC/UI 回归测试，再更新产品支持矩阵。

## 当前状态

本文件仍作为设计基线，但对应生产代码已在当前工作树实现：`src-tauri/src/session_transfer.rs`、会话管理合并逻辑和 `SessionTransferDialog.tsx` 已接入；CASR 入口位于 `vendor/casr`。实现尚未提交或发布，且上述测试/日志限制仍有效。
