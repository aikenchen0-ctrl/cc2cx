export default {
  // General
  'welcome': 'Welcome to cc-boot',
  'done': 'Done',
  'skip': 'Skip',
  'cancel': 'Cancelled',
  'error': 'Error',
  'yes': 'Yes',
  'no': 'No',

  // Language
  'lang.select': '请选择语言 / Select language',
  'lang.ai_output': 'Select AI output language',

  // Tools
  'tools.select': 'Select AI coding tools to install',
  'tools.installed': 'Installed',
  'tools.not_installed': 'Not installed',
  'tools.installing': 'Installing {{tool}}...',
  'tools.install_success': '{{tool}} installed successfully',
  'tools.install_fail': '{{tool}} installation failed',
  'tools.version': 'Version: {{version}}',

  // Provider
  'provider.select': 'Select API provider',
  'provider.api_key': 'Enter API Key',
  'provider.api_url': 'Custom API URL',
  'provider.configuring': 'Configuring {{provider}} provider...',
  'provider.done': 'Provider configuration complete',

  // MCP
  'mcp.select': 'Select MCP services to enable',
  'mcp.configuring': 'Configuring MCP services...',
  'mcp.done': 'MCP configuration complete',

  // Workflow
  'workflow.select': 'Select workflows to install',
  'workflow.installing': 'Installing workflows...',
  'workflow.done': 'Workflow installation complete',

  // cc-launch
  'cclaunch.detect': 'Detecting cc-launch...',
  'cclaunch.found': 'cc-launch is installed',
  'cclaunch.not_found': 'cc-launch not detected',
  'cclaunch.install_prompt': 'Install cc-launch? (Recommended for daily management)',
  'cclaunch.installing': 'Installing cc-launch...',
  'cclaunch.handoff': 'Handing off configuration to cc-launch...',
  'cclaunch.done': 'cc-launch configuration complete',

  // Doctor
  'doctor.title': 'Environment Diagnostics',
  'doctor.checking': 'Checking environment...',

  // Init
  'init.title': 'Initialize AI Coding Environment',
  'init.complete': 'Initialization complete!',
  'init.summary': 'Configuration Summary',

  // Menu
  'menu.title': 'cc-boot Main Menu',
  'menu.init': 'Full initialization',
  'menu.setup': 'Install single tool',
  'menu.doctor': 'Environment diagnostics',
  'menu.handoff': 'Hand off to cc-launch',
  'menu.exit': 'Exit',

  // Providers
  'providers.title': 'Available API Providers',
  'providers.none_found': 'No providers found matching the filter',
  'providers.usage_hint': 'Usage:',

  // Config
  'config.title': 'cc-boot Configuration',
  'config.empty': 'No configuration saved yet. Run cc-boot init to get started.',
  'config.file_hint': 'Config file: {{path}}',
  'config.edit_hint': 'Edit keys: cc-boot config set <key> <value>',
  'config.key_not_set': 'Key "{{key}}" is not set',
  'config.unknown_key': 'Unknown config key: "{{key}}"',
  'config.set_success': 'Set {{key}} = {{value}}',
  'config.cleared': 'Configuration cleared',
  'config.valid_keys': 'Valid keys: {{keys}}',

  // Errors
  'error.network': 'Network connection failed, check your network and proxy settings',
  'error.permission': 'Permission denied, try using sudo',
  'error.not_found': 'File or directory not found',
  'error.invalid_api_key': 'Invalid or expired API key, check your API configuration',
  'error.rate_limit': 'API rate limit exceeded, try again later',
  'error.install_failed': 'Installation failed, check your network and retry',
  'error.config_invalid': 'Configuration syntax error, check JSON/YAML format',
  'error.unknown': 'An unknown error occurred',
  'error.debug_hint': 'Tip: set CC_BOOT_DEBUG=1 to see the full error',
  'error.cancelled': 'Operation cancelled',
}
