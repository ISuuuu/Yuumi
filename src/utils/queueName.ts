import type { Composer } from "vue-i18n";

/**
 * 队列名称显示：优先 i18n 翻译。
 * 翻译防冲突纠错：如果翻译包含"云顶之弈"或"TFT"，但后端实际名称不含云顶相关字样，
 * 则说明队列 ID 发生冲突，应降级显示后端解析出的 name。
 */
export function getQueueName(
  queueId: number,
  backendName: string,
  i18n: Pick<Composer, "t" | "te">,
): string {
  const key = `gameModes.${queueId}`;
  if (i18n.te(key)) {
    const translation = i18n.t(key);
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !backendName.includes("云顶") &&
      !backendName.includes("TFT")
    ) {
      return backendName;
    }
    return translation;
  }
  return backendName;
}
