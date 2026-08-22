import { inject, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { updateConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import { useToast } from "./useToast";

/**
 * 设置页通用自动保存 composable。
 * config 为 inject("appConfig")（带 ref() 兜底），
 * autoSave 为防抖 500ms 的配置保存函数。
 */
export function useSettingsAutoSave() {
  const { t } = useI18n();
  const { showToast } = useToast();
  const config =
    inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

  // ─── 自动保存（防抖 500ms）───
  // opts 兼容直接作为事件处理器引用（n-switch/n-select 等会把事件值作为首参传入），
  // 传 { silent: true } 时等价原 skipAutoSaveToast 抑制机制：本次保存成功不弹通用提示。
  let saveDebounce: ReturnType<typeof setTimeout> | null = null;
  let skipAutoSaveToast = false;

  function autoSave(opts?: unknown) {
    if (!config.value) return;
    if (
      opts !== null &&
      typeof opts === "object" &&
      (opts as { silent?: boolean }).silent
    ) {
      skipAutoSaveToast = true;
    }
    if (saveDebounce) clearTimeout(saveDebounce);
    saveDebounce = setTimeout(async () => {
      try {
        await updateConfig(config.value!);
        if (!skipAutoSaveToast) {
          showToast(t("settings.autoSave"));
        }
        skipAutoSaveToast = false;
      } catch (e) {
        showToast(t("settings.saveFailed"), "error");
      }
    }, 500);
  }

  return { config, autoSave };
}
