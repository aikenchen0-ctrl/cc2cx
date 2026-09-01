# Agent Environment Preflight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 Agent 安装状态接口中加入 Git/WSL、npm 权限、WebView2 和 .NET 预检，并在安装前阻止必然失败的任务。

**Architecture:** 继续复用 Rust 的 `get_agent_install_statuses`、`run_agent_install` 和前端 `AgentInstallPanel`。Rust 负责所有系统检测并把结构化依赖状态随 Agent 状态返回；前端只负责渲染状态、打开固定官方入口和刷新，不执行系统命令。自动修复仅保留 Node.js 与用户目录 npm 路径，其他系统组件只提示用户处理。

**Tech Stack:** Rust/Tauri 2、`std::process::Command`、Serde、React 18、TypeScript、Vitest、Testing Library。

---

### Task 1: 扩展依赖状态模型并建立纯函数测试

**Files:**
- Modify: `src-tauri/src/commands/misc.rs:231-249`
- Test: `src-tauri/src/commands/misc.rs` 内 `commands::misc::tests` 模块
- Modify: `src/lib/api/agentInstall.ts:1-50`
- Test: `tests/lib/agentInstall.test.ts`

- [ ] **Step 1: Write the failing Rust tests**

增加纯函数测试，先定义预期行为：

```rust
#[test]
fn claude_windows_dependency_blocks_without_git_bash_or_wsl() {
    let deps = dependencies_for_agent("claude", "windows", false, false, true, true);
    assert!(deps.iter().any(|item| item.name == "Git Bash 或 WSL" && item.blocking && !item.available));
}

#[test]
fn opencode_windows_wsl_is_advisory_only() {
    let deps = dependencies_for_agent("opencode", "windows", false, false, true, true);
    assert!(deps.iter().any(|item| item.name == "WSL" && !item.blocking && !item.available));
}

#[test]
fn windows_dotnet_and_webview2_are_non_blocking_advisories() {
    let workbuddy = dependencies_for_agent("workbuddy", "windows", true, false, false, false);
    assert!(workbuddy.iter().all(|item| !item.blocking));
}
```

`dependencies_for_agent` 可以先声明为测试期望的内部函数，测试必须因函数不存在或状态字段不存在而失败。

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests::claude_windows_dependency_blocks_without_git_bash_or_wsl`

Expected: FAIL because the new dependency fields and mapping function are not implemented.

- [ ] **Step 3: Write the minimal model and mapping implementation**

将 `AgentInstallDependency` 扩展为：

```rust
#[derive(serde::Serialize)]
pub struct AgentInstallDependency {
    name: String,
    available: bool,
    detail: String,
    kind: String,
    blocking: bool,
    auto_fixable: bool,
    action: Option<String>,
}
```

新增 `dependencies_for_agent`，只负责根据已计算的布尔值组装依赖项；运行命令探测放到独立函数，避免测试依赖真实主机环境。所有新增字段使用稳定的蛇形命名，前端类型同步增加相同字段。

- [ ] **Step 4: Run focused tests to verify they pass**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests::claude_windows_dependency_blocks_without_git_bash_or_wsl commands::misc::tests::opencode_windows_wsl_is_advisory_only commands::misc::tests::windows_dotnet_and_webview2_are_non_blocking_advisories`

Expected: PASS.

- [ ] **Step 5: Add TypeScript type assertions**

在 `tests/lib/agentInstall.test.ts` 中验证 `AgentInstallDependency` 包含 `kind`、`blocking`、`autoFixable` 和 `action`，并保持已有 API 监听测试通过。

- [ ] **Step 6: Run TypeScript tests**

Run: `node .\\node_modules\\vitest\\vitest.mjs run tests/lib/agentInstall.test.ts`

Expected: PASS。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/misc.rs src/lib/api/agentInstall.ts tests/lib/agentInstall.test.ts
git commit -m "feat: add structured agent dependency status"
```

### Task 2: 实现 Windows/macOS 环境探测并接入 Agent 状态

**Files:**
- Modify: `src-tauri/src/commands/misc.rs` near `command_exists`, `agent_dependencies`, and `get_agent_install_statuses`
- Test: `src-tauri/src/commands/misc.rs` 内 `commands::misc::tests`

- [ ] **Step 1: Write failing detection tests**

覆盖以下纯函数：Git Bash 路径匹配、WSL 输出解析、npm prefix 可写结果、WebView2 注册表结果和 .NET 运行时名称解析。测试输入全部使用字符串或临时目录，不调用真实网络和真实注册表。

```rust
#[test]
fn parse_wsl_distribution_output_requires_a_distribution() {
    assert!(parse_wsl_distribution_output("Ubuntu\n").is_some());
    assert!(parse_wsl_distribution_output("\n").is_none());
}

#[test]
fn npm_prefix_writeability_rejects_read_only_result() {
    assert!(!npm_prefix_is_usable(false, false));
    assert!(npm_prefix_is_usable(true, true));
}

#[test]
fn dotnet_runtime_parser_accepts_desktop_runtime() {
    assert!(has_dotnet_runtime("Microsoft.WindowsDesktop.App 8.0.1 [C:\\Program Files\\dotnet\\shared]"));
    assert!(!has_dotnet_runtime(""));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests::parse_wsl_distribution_output_requires_a_distribution`

Expected: FAIL because the detection helpers do not exist.

- [ ] **Step 3: Implement isolated probes**

新增以下函数，所有系统调用失败均返回不可用而不是可用：

```rust
fn windows_git_bash_available() -> bool;
fn windows_wsl_distribution_available() -> bool;
fn npm_global_prefix_status() -> (bool, String);
#[cfg(target_os = "windows")]
fn windows_webview2_available() -> bool;
#[cfg(target_os = "windows")]
fn windows_dotnet_runtime_available() -> bool;
```

Git Bash 检查 `git.exe` 和常见 `bash.exe` 路径；WSL 检查 `wsl.exe --status` 以及 `wsl.exe -l -q` 是否返回发行版；npm prefix 使用 `npm config get prefix` 后在目标目录创建并删除随机临时文件；WebView2 读取 Evergreen Runtime 注册表项或安装目录；.NET 解析 `dotnet --list-runtimes`。Windows 子进程全部设置 `CREATE_NO_WINDOW`。

- [ ] **Step 4: 接入 Agent 依赖映射**

让 `agent_dependencies` 根据 Agent 标识返回：

- Claude Code：Node.js 阻塞项；Windows 增加 Git Bash 或 WSL 阻塞项。
- Codex CLI/OpenCode CLI：Node.js 阻塞项。
- OpenCode CLI Windows：WSL 建议项，不阻塞。
- WorkBuddy Windows：.NET 建议项，不阻塞。
- 当前 CLI 安装不添加 WebView2 阻塞项；为未来桌面版本保留独立探测函数。

让 `get_agent_install_statuses` 对桌面 Agent 也返回依赖项，并保持旧字段和 Agent 数量不变。

- [ ] **Step 5: Run focused Rust tests**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests`

Expected: 全部通过。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/misc.rs
git commit -m "feat: detect agent installation prerequisites"
```

### Task 3: 在安装前执行阻塞检查并支持 npm 用户目录

**Files:**
- Modify: `src-tauri/src/commands/misc.rs:1314-1408`
- Test: `src-tauri/src/commands/misc.rs` 内 `commands::misc::tests`

- [ ] **Step 1: Write failing command-gating tests**

测试 `run_agent_install` 使用预检结果时不会启动阻塞失败的子进程，并验证 npm 全局目录不可写时生成用户目录前缀命令。

```rust
#[test]
fn blocked_dependency_prevents_agent_command_start() {
    let decision = install_gate(false, true);
    assert_eq!(decision, InstallGate::Blocked);
}

#[test]
fn npm_install_command_can_use_user_prefix_without_global_registry_change() {
    let command = npm_install_command_with_user_prefix("codex", "C:\\Users\\test\\.cc-launch\\npm")
        .expect("Codex command should exist");
    assert!(command.contains("--prefix"));
    assert!(!command.contains("npm config set registry"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests::blocked_dependency_prevents_agent_command_start`

Expected: FAIL because install gating and user-prefix command are not implemented.

- [ ] **Step 3: Implement gating and prefix command**

新增 `InstallGate` 枚举和 `npm_install_command_with_user_prefix`。`run_agent_install` 在获取 Agent 规格后重新计算阻塞依赖；阻塞时发出 `phase: prepare` 的错误进度事件并返回中文错误，不创建安装子进程。npm 用户目录命令只在检测到 prefix 不可写时使用，并继续套用官方源、npmmirror、华为云的顺序回退。

- [ ] **Step 4: Run focused Rust tests**

Run: `cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests::blocked_dependency_prevents_agent_command_start commands::misc::tests::npm_install_command_can_use_user_prefix_without_global_registry_change`

Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/misc.rs
git commit -m "feat: gate installs on blocking prerequisites"
```

### Task 4: 更新安装面板的依赖状态、动作和批量跳过

**Files:**
- Modify: `src/components/agent-install/AgentInstallPanel.tsx`
- Modify: `src/components/agent-install/InstallConfirmationPanel.tsx`
- Modify: `src/lib/api/agentInstall.ts`
- Test: `tests/components/AgentInstallPanel.test.tsx`

- [ ] **Step 1: Write failing UI tests**

增加测试：阻塞依赖禁用安装按钮、建议项不禁用、可自动修复项显示修复动作、点击重新检测重新加载状态、批量安装跳过阻塞项但继续其他 Agent。

```tsx
it("disables only agents with unavailable blocking dependencies", async () => {
  render(<AgentInstallPanel isOpen onClose={() => undefined} />);
  expect(screen.getByRole("button", { name: "安装 Claude Code" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "安装 OpenCode" })).not.toBeDisabled();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `node .\\node_modules\\vitest\\vitest.mjs run tests/components/AgentInstallPanel.test.tsx`

Expected: FAIL because the panel currently only checks `command` and `installed`。

- [ ] **Step 3: Implement dependency rendering and actions**

在 Agent 卡片中按 `kind` 显示依赖状态，使用 `blocking && !available` 计算按钮禁用状态；`autoFixable` 依赖保留应用内修复动作，`action` 只用于打开固定官方说明。重新检测调用现有状态 API。批量安装在每个 Agent 执行前读取最新依赖，记录跳过原因并继续其他任务。

- [ ] **Step 4: Run UI tests**

Run: `node .\\node_modules\\vitest\\vitest.mjs run tests/components/AgentInstallPanel.test.tsx tests/lib/agentInstall.test.ts`

Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src/components/agent-install/AgentInstallPanel.tsx src/components/agent-install/InstallConfirmationPanel.tsx src/lib/api/agentInstall.ts tests/components/AgentInstallPanel.test.tsx tests/lib/agentInstall.test.ts
git commit -m "feat: show agent prerequisite states"
```

### Task 5: 完成集成验证、文档和构建检查

**Files:**
- Modify: `docs/user-manual/zh/1-getting-started/1.2-installation.md`
- Test: `tests/integration/App.test.tsx`
- Verify: `src-tauri/src/commands/misc.rs`, `src/components/agent-install/AgentInstallPanel.tsx`

- [ ] **Step 1: Add integration coverage**

验证首启进入安装面板后能够渲染依赖状态，阻塞 Agent 不会触发 `run_agent_install`，重新检测后按钮状态更新。

- [ ] **Step 2: Update user documentation**

记录 Git Bash/WSL、WebView2、.NET 和 npm 权限的处理方式，明确哪些会自动修复、哪些需要用户手动安装，并保留国内镜像顺序和安全校验说明。

- [ ] **Step 3: Run complete verification**

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
cargo +stable fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo +stable check --manifest-path src-tauri/Cargo.toml --offline
cargo +stable test --manifest-path src-tauri/Cargo.toml --offline commands::misc::tests
git diff --check
```

Expected: 所有命令退出码为 0；Vitest 全部通过；Rust `misc` 测试全部通过。既存的 Tauri mock 和 React act 警告不作为本功能失败条件，但不得新增失败测试。

- [ ] **Step 4: Build Windows bundles**

Run: `pnpm build`

Expected: 生成 `src-tauri/target/release/cc-launch.exe`、MSI 和 NSIS 安装包。若未配置 `TAURI_SIGNING_PRIVATE_KEY`，记录签名步骤失败，但保留已生成的未签名产物并明确说明。

- [ ] **Step 5: Commit documentation and integration tests**

```bash
git add docs/user-manual/zh/1-getting-started/1.2-installation.md tests/integration/App.test.tsx
git commit -m "docs: document agent environment checks"
```
