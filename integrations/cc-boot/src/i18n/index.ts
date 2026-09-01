import i18next from 'i18next'
import zhCN from './locales/zh-CN/common.js'
import en from './locales/en/common.js'

let initialized = false

export async function initI18n(lang: string = 'zh-CN'): Promise<void> {
  if (initialized)
    return

  await i18next.init({
    lng: lang,
    fallbackLng: 'en',
    resources: {
      'zh-CN': { translation: zhCN },
      en: { translation: en },
    },
    interpolation: { escapeValue: false },
    showSupportNotice: false,
  })
  initialized = true
}

export function t(key: string, options?: Record<string, any>): string {
  return i18next.t(key, options)
}

export function setLanguage(lang: string): void {
  i18next.changeLanguage(lang)
}

export { i18next }
