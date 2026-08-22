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
      // Vite 动态导入 JSON 的运行时模块形状为 { default: 语言包对象 }，TS 无法推断其 default 导出
      const raw: unknown =
        targetLocale === "zh_TW"
          ? await import("./locales/zh-TW.json")
          : targetLocale === "en_US"
            ? await import("./locales/en-US.json")
            : undefined;
      const messages = (raw as { default?: typeof zh_CN } | undefined)?.default;
      if (messages) {
        i18n.global.setLocaleMessage(targetLocale, messages);
        loadedLocales.add(targetLocale);
      }
    } catch (e) {
      console.warn(`[i18n] 加载语言包 ${targetLocale} 失败:`, e);
    }
  }
  // 与 championCache.ts 同款收敛：Composer 的 locale 被推断为字面量类型，需还原为可写字符串
  (i18n.global.locale as unknown as { value: string }).value = targetLocale;
}

export default i18n;
