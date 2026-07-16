import { createI18n } from "vue-i18n";
import zh_CN from "./locales/zh-CN.json";

// 获取浏览器语言并映射到受支持的 locale
function getBrowserLocale(): string {
  const sysLang = navigator.language || "zh-CN";
  if (
    sysLang.startsWith("zh-TW") ||
    sysLang.startsWith("zh-HK") ||
    sysLang.startsWith("zh-MO")
  ) {
    return "zh_TW";
  } else if (sysLang.startsWith("zh")) {
    return "zh_CN";
  } else {
    return "en_US";
  }
}

// 转换配置项值 (Auto / zh_CN / zh_TW / en_US) 转换为实际 locale 标识
export function getLocaleFromConfig(lang: string): string {
  if (!lang || lang === "Auto") {
    return getBrowserLocale();
  }
  return lang;
}

const i18n = createI18n({
  legacy: false, // 启用 Composition API 模式
  locale: "zh_CN", // 默认语言，App 载入配置后会覆写
  fallbackLocale: "zh_CN",
  messages: {
    zh_CN,
  },
});

// 按需加载其他语言包（首次切换时动态导入）
const loadedLocales = new Set<string>(["zh_CN"]);

export async function setLocale(lang: string) {
  const targetLocale = getLocaleFromConfig(lang);
  if (!loadedLocales.has(targetLocale)) {
    try {
      let messages: any;
      if (targetLocale === "zh_TW") {
        messages = await import("./locales/zh-TW.json");
      } else if (targetLocale === "en_US") {
        messages = await import("./locales/en-US.json");
      }
      if (messages) {
        i18n.global.setLocaleMessage(targetLocale, messages.default);
        loadedLocales.add(targetLocale);
      }
    } catch (e) {
      console.warn(`[i18n] 加载语言包 ${targetLocale} 失败:`, e);
    }
  }
  (i18n.global.locale as any).value = targetLocale;
}

export default i18n;
