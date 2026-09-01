export default {
  // General
  'welcome': '欢迎使用 cc-boot',
  'done': '完成',
  'skip': '跳过',
  'cancel': '已取消',
  'error': '错误',
  'yes': '是',
  'no': '否',

  // Language
  'lang.select': '请选择语言 / Select language',
  'lang.ai_output': '选择 AI 输出语言',

  // Tools
  'tools.select': '选择要安装的 AI 编程工具',
  'tools.installed': '已安装',
  'tools.not_installed': '未安装',
  'tools.installing': '正在安装 {{tool}}...',
  'tools.install_success': '{{tool}} 安装成功',
  'tools.install_fail': '{{tool}} 安装失败',
  'tools.version': '版本: {{version}}',

  // Provider
  'provider.select': '选择 API 供应商',
  'provider.api_key': '请输入 API Key',
  'provider.api_url': '自定义 API URL',
  'provider.configuring': '正在配置 {{provider}} 供应商...',
  'provider.done': '供应商配置完成',

  // MCP
  'mcp.select': '选择要启用的 MCP 服务',
  'mcp.configuring': '正在配置 MCP 服务...',
  'mcp.done': 'MCP 配置完成',

  // Workflow
  'workflow.select': '选择要安装的工作流',
  'workflow.installing': '正在安装工作流...',
  'workflow.done': '工作流安装完成',

  // cc-launch
  'cclaunch.detect': '检测 cc-launch...',
  'cclaunch.found': 'cc-launch 已安装',
  'cclaunch.not_found': '未检测到 cc-launch',
  'cclaunch.install_prompt': '是否安装 cc-launch？（推荐用于日常管理）',
  'cclaunch.installing': '正在安装 cc-launch...',
  'cclaunch.handoff': '正在将配置移交给 cc-launch...',
  'cclaunch.done': 'cc-launch 配置完成',

  // Doctor
  'doctor.title': '环境诊断',
  'doctor.checking': '正在检查环境...',

  // Init
  'init.title': '初始化 AI 编程环境',
  'init.complete': '初始化完成！',
  'init.summary': '配置摘要',

  // Menu
  'menu.title': 'cc-boot 主菜单',
  'menu.init': '完整初始化',
  'menu.setup': '安装单个工具',
  'menu.doctor': '环境诊断',
  'menu.handoff': '移交到 cc-launch',
  'menu.exit': '退出',

  // Providers
  'providers.title': '可用 API 供应商列表',
  'providers.none_found': '未找到匹配的供应商',
  'providers.usage_hint': '使用方式（Usage）：',

  // Config
  'config.title': 'cc-boot 配置',
  'config.empty': '尚无配置。运行 cc-boot init 开始初始化。',
  'config.file_hint': '配置文件：{{path}}',
  'config.edit_hint': '修改配置：cc-boot config set <键> <值>',
  'config.key_not_set': '键 "{{key}}" 尚未设置',
  'config.unknown_key': '未知配置键：{{key}}',
  'config.set_success': '已设置 {{key}} = {{value}}',
  'config.cleared': '配置已清除',
  'config.valid_keys': '有效键：{{keys}}',

  // Errors
  'error.network': '网络连接失败，请检查网络和代理设置',
  'error.permission': '权限不足，请尝试使用 sudo',
  'error.not_found': '找不到文件或目录',
  'error.invalid_api_key': 'API Key 无效或已过期，请检查 API 配置',
  'error.rate_limit': 'API 请求频率过高，请稍后再试',
  'error.install_failed': '安装失败，请检查网络连接后重试',
  'error.config_invalid': '配置文件格式错误，请检查 JSON/YAML 格式',
  'error.unknown': '发生未知错误',
  'error.debug_hint': '提示：设置 CC_BOOT_DEBUG=1 可查看完整错误信息',
  'error.cancelled': '操作已取消',
}
