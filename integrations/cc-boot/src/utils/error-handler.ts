import { colors, fail } from './ui.js'

/**
 * Known error categories with user-friendly messages
 */
export const ERROR_MESSAGES: Record<string, { zh: string; en: string }> = {
  ENOENT: {
    zh: '找不到文件或目录',
    en: 'File or directory not found',
  },
  EACCES: {
    zh: '权限不足，请尝试使用 sudo',
    en: 'Permission denied, try using sudo',
  },
  EEXIST: {
    zh: '文件已存在',
    en: 'File already exists',
  },
  ECONNREFUSED: {
    zh: '连接被拒绝，请检查网络',
    en: 'Connection refused, check your network',
  },
  ETIMEDOUT: {
    zh: '连接超时，请检查网络或代理',
    en: 'Connection timed out, check your network or proxy',
  },
  ENOTFOUND: {
    zh: '无法解析主机名，请检查网络连接',
    en: 'Host not found, check your network connection',
  },
  ERR_MODULE_NOT_FOUND: {
    zh: '模块未找到，请运行 npm install 重新安装依赖',
    en: 'Module not found, run npm install to reinstall dependencies',
  },
}

/**
 * Keyword patterns → friendly message mapping
 */
const MESSAGE_PATTERNS: Array<{
  pattern: RegExp
  zh: string
  en: string
}> = [
  {
    pattern: /npm (install|ci|publish)/i,
    zh: 'npm 操作失败，请检查网络连接和 npm 配置',
    en: 'npm operation failed, check network and npm config',
  },
  {
    pattern: /permission denied/i,
    zh: '权限不足，请使用 sudo 或检查文件权限',
    en: 'Permission denied, use sudo or check file permissions',
  },
  {
    pattern: /command not found/i,
    zh: '命令未找到，请检查工具是否正确安装',
    en: 'Command not found, check if the tool is properly installed',
  },
  {
    pattern: /network|ECONNREFUSED|ETIMEDOUT|ENOTFOUND/i,
    zh: '网络连接失败，请检查网络和代理设置',
    en: 'Network connection failed, check your network and proxy settings',
  },
  {
    pattern: /ENOMEM|out of memory/i,
    zh: '内存不足，请关闭其他程序后重试',
    en: 'Out of memory, close other programs and try again',
  },
  {
    pattern: /SyntaxError/i,
    zh: '配置文件格式错误，请检查 JSON/YAML 格式',
    en: 'Configuration syntax error, check JSON/YAML format',
  },
  {
    pattern: /Invalid API key|Unauthorized|401/i,
    zh: 'API Key 无效或已过期，请检查 API 配置',
    en: 'Invalid or expired API key, check your API configuration',
  },
  {
    pattern: /rate limit|429/i,
    zh: 'API 请求过于频繁，请稍后再试',
    en: 'API rate limit exceeded, try again later',
  },
]

/**
 * Get a user-friendly message for an error
 */
export function getFriendlyMessage(err: unknown): { message: string; hint?: string } {
  const e = err as any
  const rawMessage: string = e?.message || String(err) || 'Unknown error'
  const code: string = e?.code || ''

  // Check by error code first
  if (code && ERROR_MESSAGES[code]) {
    const lang = detectLang()
    const entry = ERROR_MESSAGES[code]
    return {
      message: lang === 'zh' ? entry.zh : entry.en,
      hint: rawMessage !== entry.zh && rawMessage !== entry.en ? rawMessage : undefined,
    }
  }

  // Check by message pattern
  for (const { pattern, zh, en } of MESSAGE_PATTERNS) {
    if (pattern.test(rawMessage)) {
      const lang = detectLang()
      return {
        message: lang === 'zh' ? zh : en,
        hint: rawMessage,
      }
    }
  }

  // Fallback: return raw message
  return { message: rawMessage }
}

/**
 * Simple locale detection — check env vars or default to zh
 */
function detectLang(): 'zh' | 'en' {
  const lang = process.env.LANG || process.env.LC_ALL || process.env.LC_MESSAGES || ''
  return lang.startsWith('en') ? 'en' : 'zh'
}

/**
 * Print a friendly error and exit
 */
export function handleFatalError(err: unknown, context?: string): never {
  console.log()
  const { message, hint } = getFriendlyMessage(err)

  if (context) {
    fail(`${context}: ${message}`)
  } else {
    fail(message)
  }

  if (hint && hint !== message) {
    console.log(colors.dim(`  → ${hint}`))
  }

  // In debug mode, print full stack
  if (process.env.CC_BOOT_DEBUG) {
    console.log()
    console.log(colors.dim('  [debug] Full error:'))
    console.error(err)
  } else {
    console.log(colors.dim('  提示：设置 CC_BOOT_DEBUG=1 可查看完整错误信息'))
  }

  console.log()
  process.exit(1)
}

/**
 * Wrap an async command action with error handling
 */
export function withErrorHandler<T extends unknown[]>(
  context: string,
  fn: (...args: T) => Promise<void>,
): (...args: T) => Promise<void> {
  return async (...args: T) => {
    try {
      await fn(...args)
    } catch (err) {
      handleFatalError(err, context)
    }
  }
}

/**
 * Register global uncaught exception handlers
 */
export function registerGlobalErrorHandlers(): void {
  process.on('uncaughtException', (err) => {
    handleFatalError(err, '未捕获的异常')
  })

  process.on('unhandledRejection', (reason) => {
    handleFatalError(reason, '未处理的 Promise 错误')
  })

  // Graceful SIGINT (Ctrl+C)
  process.on('SIGINT', () => {
    console.log()
    console.log(colors.dim('  已取消'))
    console.log()
    process.exit(0)
  })
}
