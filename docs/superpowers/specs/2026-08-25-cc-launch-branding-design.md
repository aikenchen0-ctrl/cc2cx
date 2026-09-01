# cc-launch 品牌与三端隔离设计

## 目标

将桌面应用及其 cc-boot 集成统一更名为 `cc-launch`，同时作为一个独立的新应用发布。Windows、macOS 与 Linux 上的新版本必须使用独立标识、独立本地数据与独立同步空间；不读取、迁移或写入旧版 `cc-switch` 数据。

## 范围

本次改动包括：

- 用户可见名称：窗口标题、安装程序、应用包、cc-boot 文案与项目元数据。
- 稳定标识：Tauri bundle identifier、深链 scheme、Flatpak application ID、三端包名和发布工作流中的产物名。
- 本应用数据：数据库、配置、备份、技能 SSOT、WebDAV/S3 默认同步根目录与同步格式。
- cc-boot：三端安装、已安装检测、深链交接及相应测试。
- CI：覆盖 Windows、macOS、Linux 的构建或检查入口，明确各平台包产物。

以下内容不在范围内：

- 从 `cc-switch` 自动导入账号、供应商、MCP、提示词、技能、数据库或同步数据。
- 兼容或注册 `ccswitch://` 深链。
- 变更 Claude、Codex、Gemini 等受管理 Agent 的原生配置目录。
- 机械替换代理请求中的 wire-format 标记、历史回归测试标记、第三方优惠码、外部仓库 URL 或线上更新地址。它们逐项审查，只有确定由新应用拥有时才改名。

## 应用身份与数据边界

新应用使用以下身份：

| 项目 | 新值 |
| --- | --- |
| 展示名称 | `cc-launch` |
| Tauri 标识 | `com.cclaunch.desktop` |
| 深链 | `cclaunch://` |
| 本地状态目录 | `~/.cc-launch/` |
| 数据库文件 | `~/.cc-launch/cc-launch.db` |
| 默认同步根目录 | `cc-launch-sync` |
| 同步 manifest 格式 | `cc-launch-webdav-sync` |

所有由 `get_app_config_dir()` 派生的路径都落在新目录，包括配置、备份、技能和数据库。旧 `~/.cc-switch/` 在本版本中不被探测、读取、写入或删除，因而两个应用可以并存。

外部 Agent 的配置目录仍按平台与 Agent 的现有规则处理，例如 `~/.claude`、`~/.codex`。它们不是 cc-switch 私有数据；`cc-launch` 在用户明确执行配置操作时仍可写入这些 Agent 的标准配置位置。

## 构建与安装

Tauri 基础配置、Windows WiX 模板、Cargo 二进制名称、Flatpak 元数据和发布资产名称一致采用 `cc-launch`。cc-boot 的安装器、探测器和深链生成器改为针对新应用，并使用其发布渠道和三端包名。

三端策略：

- Windows：WiX/NSIS 产物、卸载项、安装目录与可执行文件使用 cc-launch 身份。
- macOS：应用 bundle 标识和 cask/DMG 名称使用 cc-launch 身份，维持最低 macOS 12 要求。
- Linux：Deb、RPM、AppImage 和 Flatpak ID/desktop entry 使用 cc-launch 身份。

发布流程不得假定不同平台可共享二进制；每个平台由对应 runner 生成本机包。跨平台源码检查可在任意 runner 执行，但打包验证必须在目标平台运行。

## 错误处理与兼容性

路径或打包配置错误必须明确报错，不引入静默回退到 `cc-switch` 数据目录、旧深链或旧远程同步目录的逻辑。这样用户能够明确看到新应用的配置失败，而不是误以为新旧数据混用。

代理转换与同步解析中的既有内部标记在没有对应的数据升级设计前保持不变。同步格式改为新值后，旧远程空间会被视为不同格式而拒绝使用，避免新应用误写旧数据。

## 验证

- Rust 单元与集成测试覆盖新数据目录、数据库文件、同步默认值及新 deep link。
- cc-boot 单元测试覆盖新深链、安装检测路径和三端安装资产选择。
- 前端类型检查与单元测试通过。
- Windows 本地执行完整构建并检查生成的可执行文件和安装包名称。
- CI 在 Windows、macOS、Linux runner 上执行对应的构建或打包检查；本地不将未运行的平台宣称为已验证。

## 验收标准

首次启动 `cc-launch` 后，仅在 `~/.cc-launch/` 创建本应用数据。旧 `~/.cc-switch/` 保持字节级不变。新建同步配置默认连接到 `cc-launch-sync`，新的 cc-boot 深链以 `cclaunch://` 开头，并且三端发布流程不再依赖旧应用名作为产物或安装身份。
