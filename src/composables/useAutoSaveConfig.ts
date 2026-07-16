import { inject, type Ref } from "vue";
import { updateConfig as persistConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";

/**
 * 工具页面卡片通用的自动保存配置 composable。
 * 从 provide/inject 获取本地配置 ref 和同步函数，
 * 返回 triggerAutoSave 供各卡片调用。
 */
export function useAutoSaveConfig() {
  const config = inject<Ref<AppConfig | null>>("appConfig");
  const syncLocalConfig = inject<(config: AppConfig) => void>("updateConfig");

  async function triggerAutoSave() {
    if (!config?.value || !syncLocalConfig) return;
    const newConfig = { ...config.value };
    newConfig.Functions = { ...newConfig.Functions };
    // 同步自动选用开关和自动亮起开关
    newConfig.Functions.EnableAutoSelectChampion =
      newConfig.Functions.EnableAutoHoverChampion;
    syncLocalConfig(newConfig);
    try {
      await persistConfig(newConfig);
    } catch (e) {
      console.error("自动保存设置失败:", e);
    }
  }

  return { config, triggerAutoSave };
}
