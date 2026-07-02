import { createI18n } from 'vue-i18n';
import zh_CN from './locales/zh-CN.json';
import zh_TW from './locales/zh-TW.json';
import en_US from './locales/en-US.json';

// 获取浏览器语言并映射到受支持的 locale
function getBrowserLocale(): string {
  const sysLang = navigator.language || 'zh-CN';
  if (sysLang.startsWith('zh-TW') || sysLang.startsWith('zh-HK') || sysLang.startsWith('zh-MO')) {
    return 'zh_TW';
  } else if (sysLang.startsWith('zh')) {
    return 'zh_CN';
  } else {
    return 'en_US';
  }
}

// 转换配置项值 (Auto / zh_CN / zh_TW / en_US) 转换为实际 locale 标识
export function getLocaleFromConfig(lang: string): string {
  if (!lang || lang === 'Auto') {
    return getBrowserLocale();
  }
  return lang;
}

const i18n = createI18n({
  legacy: false, // 启用 Composition API 模式
  locale: 'zh_CN', // 默认语言，App 载入配置后会覆写
  fallbackLocale: 'zh_CN',
  messages: {
    zh_CN,
    zh_TW,
    en_US,
  },
});

// 提供即时切换的辅助方法
export function setLocale(lang: string) {
  const targetLocale = getLocaleFromConfig(lang);
  (i18n.global.locale as any).value = targetLocale;
}

export default i18n;
