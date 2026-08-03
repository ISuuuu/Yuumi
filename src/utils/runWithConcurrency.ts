/**
 * 以固定并发上限执行异步任务，避免一次性发起过多网络请求造成请求风暴。
 */
export async function runWithConcurrency<T>(
  items: T[],
  limit: number,
  worker: (item: T) => Promise<void> | void,
): Promise<void> {
  if (items.length === 0) return;
  const workers = Math.max(1, Math.min(limit, items.length));

  let index = 0;
  const run = async () => {
    while (index < items.length) {
      const current = index++;
      await worker(items[current]);
    }
  };

  await Promise.all(Array.from({ length: workers }, () => run()));
}
