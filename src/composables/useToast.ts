import { useDialog, useMessage } from "naive-ui";

/** Naive UI Message API 类型（从 useMessage 推导，避免深路径导入） */
export type MessageApi = ReturnType<typeof useMessage>;
/** Naive UI Dialog API 类型 */
export type DialogApi = ReturnType<typeof useDialog>;

/**
 * Provider 内部捕获的 Naive UI API 实例（模块级共享）。
 * 由 <NaiveApiCapture /> 在 Provider 子树内调用捕获函数写入，
 * 供位于 Provider 之外的调用方（如 App.vue 自身）使用。
 */
let capturedMessage: MessageApi | null = null;
let capturedDialog: DialogApi | null = null;

/** 由 Provider 内部的捕获组件调用，登记当前窗口的 Message 实例 */
export function captureMessageApi(api: MessageApi) {
  capturedMessage = api;
}

/** 获取捕获的 Dialog 实例（无 Provider 上下文且未捕获时为 null） */
export function getCapturedDialog() {
  return capturedDialog;
}

/** 由 Provider 内部的捕获组件调用，登记当前窗口的 Dialog 实例 */
export function captureDialogApi(api: DialogApi) {
  capturedDialog = api;
}

/**
 * Naive UI 消息提示 Hook
 * 只能在 NMessageProvider 的子组件中使用（App.vue 已包装 Provider）
 * 在没有 Provider 的场景中进行了优雅安全降级，防止主渲染树或独立 Webview 崩溃
 */
export function useToast() {
  let message: MessageApi | null = null;
  try {
    message = useMessage();
  } catch {
    // 无 Provider 上下文（如 App.vue 自身 setup），回退到捕获实例
  }

  function showToast(msg: string, type: "success" | "error" | "warning" | "info" = "success") {
    const activeMessage = message ?? capturedMessage;
    if (activeMessage) {
      if (type === "error") {
        activeMessage.error(msg);
      } else if (type === "warning") {
        activeMessage.warning(msg);
      } else if (type === "info") {
        activeMessage.info(msg);
      } else {
        activeMessage.success(msg);
      }
    } else {
      console.warn(`[Toast 降级 - ${type}]: ${msg}`);
    }
  }

  return { showToast };
}
