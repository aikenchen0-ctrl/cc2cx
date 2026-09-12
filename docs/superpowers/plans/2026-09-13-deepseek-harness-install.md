# DeepSeek Harness 自动安装实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add DeepSeek Harness CLI and its ACP plugin as a verifiable, automatically installable Agent in the existing Agent installation panel.

**Architecture:** Extend the existing Rust AgentInstallSpec and npm tool registry with `deepseek`/`dsh`. Build one platform-neutral npm install command for the pinned CLI, then register the pinned ACP plugin in the same install task; reuse current Node.js preflight, npm user-prefix handling, registry fallbacks, progress events, and post-install executable probing.

**Tech Stack:** Rust/Tauri, existing `commands/misc.rs` Agent installer, npm, `dsh` CLI, DeepSeek ACP plugin, Vitest/React existing Agent install panel.

---

## File Map

- Modify: `src-tauri/src/commands/misc.rs` — add DeepSeek spec, `dsh` to tool registry/search/version probing, pinned npm/ACP command, status capacity, dependency gating, display labels, and tests.
- Modify: `src/lib/api/agentInstall.ts` — add DeepSeek usage metadata only if the existing typed usage map requires an explicit key.
- Modify: `src/components/agent-install/AgentInstallPanel.tsx` — add DeepSeek usage hint only if the panel does not render generic status correctly; preserve generic install flow.
- Modify: `src/i18n/locales/zh.json`, `src/i18n/locales/en.json`, `src/i18n/locales/ja.json`, `src/i18n/locales/zh-TW.json` — add labels only if the panel uses locale keys for Agent names.
- Create: `src-tauri/tests/deepseek_harness_install.rs` — public-contract tests for status/spec/command behavior if private functions cannot be covered from the existing module test block.
- Modify: `docs/superpowers/specs/2026-09-13-deepseek-harness-install-design.md` — record implementation evidence after tests.
- Modify: `docs/cursor/cc2cx-cursor-handoff-2026-09-08.md` — add the new Agent to the handoff map and current capability list.

## Task 1: Add the DeepSeek Agent Contract

- [ ] **Step 1: Write the failing Rust tests** in the existing `misc.rs` test module or `src-tauri/tests/deepseek_harness_install.rs`:

```rust
#[test]
fn deepseek_harness_is_a_supported_node_agent() {
    let spec = agent_install_specs()
        .into_iter()
        .find(|spec| spec.id == "deepseek")
        .expect("DeepSeek Harness must be listed");
    assert_eq!(spec.tool, Some("dsh"));
    assert!(spec.supported && !spec.desktop);
}

#[test]
fn deepseek_harness_install_command_contains_pinned_cli_and_acp_packages() {
    let command = agent_install_command("dsh").expect("dsh install command");
    assert!(command.contains("@deepseek-ai/dsh@0.1.1-rc.2"));
    assert!(command.contains("@deepseek-ai/dsh-acp@0.1.1-rc.2"));
    assert!(command.contains("plugin --profile cc2cx add"));
}
```

- [ ] **Step 2: Run the tests and verify they fail.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml deepseek_harness -- --nocapture`

Expected: FAIL because no `deepseek` spec or `dsh` package mapping exists.

- [ ] **Step 3: Implement the minimal contract.**

Add one `AgentInstallSpec` entry:

```rust
AgentInstallSpec {
    id: "deepseek",
    name: "DeepSeek Harness",
    description: "DeepSeek 的 ACP 编程 Agent",
    tool: Some("dsh"),
    desktop: false,
    supported: true,
    unsupported_reason: None,
}
```

Add constants and command construction:

```rust
const DEEPSEEK_HARNESS_VERSION: &str = "0.1.1-rc.2";
const DEEPSEEK_HARNESS_PACKAGE: &str = "@deepseek-ai/dsh";
const DEEPSEEK_ACP_PACKAGE: &str = "@deepseek-ai/dsh-acp";
```

The generated install command must run CLI installation first and only then execute `dsh plugin --profile cc2cx add "@deepseek-ai/dsh-acp@0.1.1-rc.2"`. Keep it in the existing npm registry fallback chain; do not add API key or credential writes.

- [ ] **Step 4: Run the focused tests and verify they pass.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml deepseek_harness -- --nocapture`

- [ ] **Step 5: Commit.** `git add src-tauri/src/commands/misc.rs src-tauri/tests/deepseek_harness_install.rs && git commit -m "feat: add deepseek harness agent install contract"`

## Task 2: Detection and Dependency Integration

- [ ] **Step 1: Write failing tests** for `dsh` being included in `VALID_TOOLS`, display name resolution, Node.js dependency status, and version-source selection.

```rust
#[test]
fn dsh_is_in_tool_registry_and_uses_node_dependency() {
    assert!(VALID_TOOLS.contains(&"dsh"));
    assert_eq!(tool_display_name("dsh"), "DeepSeek Harness");
    let deps = dependencies_for_agent("dsh", "windows", false, false, true, true, false);
    assert!(deps.iter().any(|dep| dep.name == "Node.js" && dep.available));
}
```

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml dsh_is_in_tool_registry -- --nocapture`

- [ ] **Step 3: Implement detection.**

Add `dsh` to `VALID_TOOLS`, `npm_install_command_for`, `tool_display_name`, and the `latest_version` match. The existing `tool_executable_candidates` logic must find `dsh.cmd`/`dsh.exe` in PATH and the configured user npm prefix. Add `dsh` to the Node.js-required branch in `run_agent_install`.

- [ ] **Step 4: Run detection and existing installer tests.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml agent_install_specs dsh_is_in_tool_registry -- --nocapture`

- [ ] **Step 5: Commit.** `git add src-tauri/src/commands/misc.rs && git commit -m "feat: detect deepseek harness installation"`

## Task 3: ACP Profile Registration and Failure Semantics

- [ ] **Step 1: Write the failing command-shape test** asserting the ACP registration is chained after CLI install and that the profile name is `cc2cx`.

```rust
#[test]
fn deepseek_acp_registration_is_after_cli_install_and_uses_cc2cx_profile() {
    let command = npm_install_command_for("dsh").unwrap();
    assert!(command.find("@deepseek-ai/dsh@").unwrap() < command.find("plugin --profile cc2cx add").unwrap());
    assert!(command.contains("@deepseek-ai/dsh-acp@0.1.1-rc.2"));
}
```

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml deepseek_acp_registration -- --nocapture`

- [ ] **Step 3: Implement explicit chaining.**

Use a shell-safe command chain that returns non-zero if either npm install or ACP registration fails. On Windows batch, preserve `call` semantics for `npm.cmd` and `dsh.cmd`; on POSIX, use `&&` inside the existing command string. Do not use `||` between CLI install and plugin registration, because a successful plugin registration must not hide a failed CLI install.

- [ ] **Step 4: Add post-install verification.** After `run_agent_install_with_output` succeeds, existing status refresh must require a runnable `dsh`; a plugin registration failure must propagate as an install error and must not return `success: true`.

- [ ] **Step 5: Run focused tests.** `cargo test --locked --manifest-path src-tauri/Cargo.toml deepseek -- --nocapture`

- [ ] **Step 6: Commit.** `git add src-tauri/src/commands/misc.rs src-tauri/tests/deepseek_harness_install.rs && git commit -m "feat: register deepseek harness acp profile"`

## Task 4: UI and Documentation

- [ ] **Step 1: Write/update frontend tests** so the Agent panel renders DeepSeek Harness from backend status and opens the existing confirmation flow. The test must assert the panel does not invent a second install mechanism.

```tsx
expect(screen.getByText("DeepSeek Harness")).toBeInTheDocument();
expect(screen.getByRole("button", { name: /安装 DeepSeek Harness/i })).toBeInTheDocument();
```

- [ ] **Step 2: Run and verify the frontend test fails** if the current fixture does not include the new Agent.

Run: `pnpm vitest run tests/components/AgentInstallPanel.test.tsx`

- [ ] **Step 3: Update only the necessary UI metadata.** The generic panel should consume the new backend item without layout changes. Add a concise DeepSeek usage hint if the existing `AGENT_USAGE` map requires it; otherwise no frontend code change is needed.

- [ ] **Step 4: Update documentation.** Record fixed package versions, ACP profile registration, Node.js prerequisite, and the fact that API key configuration remains a separate user action. Update the handoff document's Agent count from 11 to 12.

- [ ] **Step 5: Run frontend and Markdown checks.**

Run: `pnpm vitest run tests/components/AgentInstallPanel.test.tsx`

Run: `git diff --check`

- [ ] **Step 6: Commit.** `git add src docs && git commit -m "docs: document deepseek harness installation"`

## Task 5: Full Verification and Optional Real Install

- [ ] **Step 1: Run Rust formatting and targeted tests.**

Run: `cargo fmt --all -- --check`

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml agent_install_specs deepseek -- --test-threads=1`

- [ ] **Step 2: Build Debug.** `cargo build --locked --manifest-path src-tauri/Cargo.toml --bin cc-launch`

- [ ] **Step 3: Verify command/status without installing** by opening Agent Install and checking that DeepSeek Harness shows Node.js dependency, pinned command, and a not-installed status when `dsh` is absent.

- [ ] **Step 4: Perform real installation only after the user explicitly asks to install.** The command installs the pinned official packages and ACP profile; it does not write API keys. Verify with `dsh --version` and the backend status refresh.

- [ ] **Step 5: Inspect the diff for accidental credentials, arbitrary shell downloads, unrelated Agent changes, or false-success paths.**

- [ ] **Step 6: Commit final verification evidence.** `git add docs && git commit -m "test: verify deepseek harness agent install"`

## Safety Gates

- Reuse the existing npm/Node dependency and output-event infrastructure.
- Keep the package version pinned at `0.1.1-rc.2` for this feature.
- Do not write `DEEPSEEK_API_KEY`, `.dsh` credentials, or any user secret.
- Do not mark the Agent installed unless `dsh` is runnable and ACP registration succeeded.
- Do not change unrelated Agent installation behavior.
