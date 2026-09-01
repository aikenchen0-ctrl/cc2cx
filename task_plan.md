# CC Father 全量合并规划

## 目标
重新梳理并规划 cc-boot、cc-switch 的全部重点能力，目标是 Electron 版本一比一覆盖，而非 MVP 裁剪。

## 阶段
- [x] 1. 盘点参考项目结构、README 能力和技术边界
- [x] 2. 设计全量能力矩阵、目标架构和迁移顺序
- [x] 3. 重写《文档合并计划.md》
- [ ] 4. 用户评审规划，确认一比一范围与版本基线
- [ ] 5. 按模块实施和建立差异验收

- [x] 4. 将 cc-switch 提升为根目录主项目并移除 Electron 方案

## 当前决策
- cc-switch 的 Tauri + React + Rust 是目标运行时；不再迁移 Electron。
- `integrations/cc-boot` 作为 pnpm workspace 子包接入主项目。
- 一比一指功能、数据行为、错误处理和关键交互覆盖；实现技术可因 Electron 平台调整。
- 以当前仓库快照作为能力基线，后续升级必须通过兼容矩阵和回归测试。

## 错误记录
暂无。
