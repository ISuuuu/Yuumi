/**
 * localStorage 延迟写入工具：避免在渲染关键路径上同步 JSON.stringify + setItem
 * 阻塞主线程。将序列化与写入推迟到浏览器空闲时段执行。
 */
export function lazySetItem(key: string, value: unknown) {
  const run = () => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* ignore */
    }
  };

  const idle = (window as unknown as { requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => void })
    .requestIdleCallback;
  if (typeof idle === "function") {
    idle(run, { timeout: 2000 });
  } else {
    setTimeout(run, 0);
  }
}
