# Tauri 版本对齐实施计划

> **执行说明：** 本计划用于记录已批准的方案 A；每一步都必须以命令输出验证后再进入下一步。

**目标：** 对齐 Tauri 前端、Rust 和打包 CLI 的直接依赖，消除打包器与二进制标记版本错配。

**方案：** 以当前 `Cargo.lock` 的直接 Tauri 版本为基准，将 `package.json` 与 `src-tauri/Cargo.toml` 中的直接依赖改为精确版本；重新生成 `pnpm-lock.yaml`，保留现有 `Cargo.lock` 可复现解析结果。业务代码不改，最终用类型检查、格式检查、完整单元测试和 NSIS 构建验证。

**验收条件：** `pnpm tauri info` 不再报告 Tauri 核心、Updater、Dialog 的 major/minor 版本错配；完整前端测试和新增 Rust 测试通过；使用项目 CLI 构建 NSIS 时不再出现 bundle 类型标记警告。

---

## 执行步骤

- [x] 将前端直接 Tauri 依赖锁定到 Rust 侧对应版本。
- [x] 将 Rust 直接 Tauri 依赖锁定到当前 `Cargo.lock` 版本。
- [x] 只更新前端锁文件并检查版本解析结果。
- [x] 运行类型检查、格式检查、完整单元测试和 Rust 目标测试。
- [x] 使用项目内 CLI 构建 NSIS，确认 bundle 类型 patch 成功。
- [x] 检查差异、提交并推送，记录构建产物校验值。
