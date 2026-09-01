# Agent 首次启动安装引导实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有 Tauri + React 应用中加入首启 Agent 自动检查、Node.js 依赖补齐和批量安装引导。

**Architecture:** 复用现有 `get_agent_install_statuses`、`run_agent_install` 和实时输出事件；新增结构化 Node.js 运行时检查/安装命令，并在前端以独立的首启协调器触发面板。桌面 Agent 仍由现有工具生命周期探测，无法自动安装的项目保持明确的官方安装提示。

**Tech Stack:** React 18、TypeScript、Vitest、Tauri 2、Rust、Tokio。

---

### Task 1: 增加可测试的首启协调逻辑

**Files:**
- Create: `src/lib/agentFirstLaunch.ts`
- Test: `tests/lib/agentFirstLaunch.test.ts`

- [ ] **Step 1: 写失败测试**

测试 `shouldOpenAgentInstallOnFirstScan`：未写入标记且状态包含缺失 Agent 时返回 `true`；测试 `shouldSkipRepeatedScan`：标记存在时返回 `false`。

- [ ] **Step 2: 运行测试确认失败**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/lib/agentFirstLaunch.test.ts`

预期：因 `src/lib/agentFirstLaunch.ts` 不存在而失败。

- [ ] **Step 3: 实现最小逻辑**

导出固定存储键、目标 Agent 过滤函数和首启判定函数；判定缺失、不可运行或缺少依赖时打开引导。

- [ ] **Step 4: 运行测试确认通过**

运行同一命令，预期全部通过。

- [ ] **Step 5: 提交**

```bash
git add src/lib/agentFirstLaunch.ts tests/lib/agentFirstLaunch.test.ts
git commit -m "feat: add first-launch agent check coordinator"
```

### Task 2: 增加 Node.js 运行时 API 与后端命令

**Files:**
- Modify: `src/lib/api/agentInstall.ts`
- Modify: `src-tauri/src/commands/misc.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `tests/lib/agentInstall.test.ts`

- [ ] **Step 1: 写失败测试**

测试 API 暴露 `ensureNodeRuntime`，并验证 Node.js 结果类型包含 `available`、`version`、`installed`、`command` 和 `error`。

- [ ] **Step 2: 运行测试确认失败**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/lib/agentInstall.test.ts`

预期：因 API 方法和类型不存在而失败。

- [ ] **Step 3: 实现 Rust 运行时探测和安装**

新增 `NodeRuntimeStatus`、`ensure_node_runtime` 与平台命令生成函数：Node.js 18+ 直接返回；Windows 优先 winget，macOS 优先 Homebrew；安装过程复用 `agent-install-output`，使用 `node-runtime` 作为事件 ID。

- [ ] **Step 4: 注册 Tauri 命令并实现前端 API**

将命令注册到 `src-tauri/src/lib.rs`，在 TypeScript API 中增加对应类型和调用方法。

- [ ] **Step 5: 运行前端测试确认通过**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/lib/agentInstall.test.ts`

- [ ] **Step 6: 提交**

```bash
git add src/lib/api/agentInstall.ts src-tauri/src/commands/misc.rs src-tauri/src/lib.rs tests/lib/agentInstall.test.ts
git commit -m "feat: add node runtime ensure command"
```

### Task 3: 让 Agent 安装在缺少 Node.js 时先补齐依赖

**Files:**
- Modify: `src/lib/api/agentInstall.ts`
- Modify: `src/components/agent-install/AgentInstallPanel.tsx`
- Modify: `src/components/agent-install/InstallConfirmationPanel.tsx`
- Test: `tests/components/AgentInstallPanel.test.tsx`

- [ ] **Step 1: 写失败测试**

覆盖 Node.js 缺失时面板显示依赖状态和“一键安装缺失项”，并验证批量流程先调用 `ensureNodeRuntime` 再调用依赖 Agent 的 `runInstall`。

- [ ] **Step 2: 运行测试确认失败**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/components/AgentInstallPanel.test.tsx`

预期：找不到新按钮或调用顺序不符合预期。

- [ ] **Step 3: 实现批量安装和依赖状态**

面板加载状态时提取 Node.js 依赖；增加批量按钮与批量结果；单项安装在 Node.js 缺失且 Agent 依赖 Node.js 时先调用运行时确保命令。

- [ ] **Step 4: 运行测试确认通过**

运行同一命令，预期全部通过。

- [ ] **Step 5: 提交**

```bash
git add src/lib/api/agentInstall.ts src/components/agent-install/AgentInstallPanel.tsx src/components/agent-install/InstallConfirmationPanel.tsx tests/components/AgentInstallPanel.test.tsx
git commit -m "feat: install node before dependent agents"
```

### Task 4: 首次启动自动打开安装引导

**Files:**
- Modify: `src/App.tsx`
- Test: `tests/integration/App.test.tsx`

- [ ] **Step 1: 写失败测试**

增加首启场景：mock Agent 状态包含缺失项且没有首启标记，渲染应用后断言安装面板标题出现；已有标记时断言不自动打开。

- [ ] **Step 2: 运行测试确认失败**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/integration/App.test.tsx`

预期：首启场景无法找到安装面板标题。

- [ ] **Step 3: 接入首启检查**

在应用初始化阶段调用 Agent 状态查询；根据协调逻辑打开面板，完成扫描后写入标记；查询失败时不阻塞主界面。

- [ ] **Step 4: 运行测试确认通过**

运行同一命令，预期新增场景通过，既有场景无回归。

- [ ] **Step 5: 提交**

```bash
git add src/App.tsx tests/integration/App.test.tsx
git commit -m "feat: open agent installer on first launch"
```

### Task 5: 完整验证

**Files:**
- Modify: 无

- [ ] **Step 1: 运行相关前端测试**

运行：`node .\\node_modules\\vitest\\vitest.mjs run tests/lib/agentFirstLaunch.test.ts tests/lib/agentInstall.test.ts tests/components/AgentInstallPanel.test.tsx tests/integration/App.test.tsx`

- [ ] **Step 2: 运行 TypeScript 检查**

运行：`pnpm exec tsc --noEmit`

若 pnpm 供应链钩子阻止执行，改用 `node .\\node_modules\\typescript\\bin\\tsc --noEmit` 并记录结果。

- [ ] **Step 3: 运行 Rust 格式与测试**

运行：`cargo fmt --check` 与 `cargo test --lib commands::misc`

- [ ] **Step 4: 检查差异和提交状态**

运行：`git diff --check`、`git status --short`，确认无意外生成文件。
