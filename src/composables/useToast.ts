import { useMessage } from "naive-ui";

/**
 * Naive UI 消息提示 Hook
 * 只能在 NMessageProvider 的子组件中使用（App.vue 已包装 Provider）
 * 在没有 Provider 的场景中进行了优雅安全降级，防止主渲染树或独立 Webview 崩溃
 */
export function useToast() {
  let message: any = null;
  try {
    message = useMessage();
  } catch (e) {
    // 忽略错误，防止无 Provider 时 setup 直接报错崩溃
  }

  function showToast(msg: string, type: "success" | "error" = "success") {
    // 优先使用上下文注入的 message，其次尝试全局挂载的 $message 实例，最后静默降级为控制台日志
    const activeMessage = message || (window as any).$message;
    if (activeMessage) {
      if (type === "error") {
        activeMessage.error(msg);
      } else {
        activeMessage.success(msg);
      }
    } else {
      console.warn(`[Toast 降级 - ${type}]: ${msg}`);
    }
  }

  return { showToast };
}
