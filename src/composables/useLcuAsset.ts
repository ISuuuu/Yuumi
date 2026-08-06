import { ref, watch, type Ref } from "vue";
import { fetchLcuAssets } from "../api/lcu";

// LRU 缓存：Map 保持插入顺序，超过上限时淘汰最久未使用的条目
const MAX_CACHE_SIZE = 500;
const cache = new Map<string, string>();

// 与 Rust 侧白名单保持一致：仅这些路径需要走 LCU 资源加载
function isLcuAssetPath(path: string): boolean {
  return (
    path.startsWith("/lol-game-data/assets/") ||
    path.startsWith("/fe/lol-loot/assets/") ||
    path.startsWith("http://") ||
    path.startsWith("https://")
  );
}

function cacheSet(key: string, value: string) {
  if (cache.has(key)) {
    cache.delete(key); // 移到最新位置
  } else if (cache.size >= MAX_CACHE_SIZE) {
    // 淘汰最旧的条目（Map 迭代器第一个）
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(key, value);
}

// 同一渲染批次内多个不同路径合并为一次 get_lcu_assets 调用，
// 显著减少高频图标场景（战绩列表 / 对局详情）下的 IPC 往返次数。
const inflight = new Map<string, Promise<string>>();

interface PendingItem {
  resolve: (dataUrl: string) => void;
  reject: (err: unknown) => void;
}

let pendingPaths = new Map<string, PendingItem[]>();
let flushScheduled = false;

function scheduleFlush() {
  if (flushScheduled) return;
  flushScheduled = true;
  setTimeout(flushPending, 0);
}

function flushPending() {
  flushScheduled = false;
  if (pendingPaths.size === 0) return;

  const batch = pendingPaths;
  pendingPaths = new Map();

  const paths = [...batch.keys()];
  const toFetch: string[] = [];

  for (const path of paths) {
    const items = batch.get(path)!;
    // 已缓存 — 直接分发
    if (cache.has(path)) {
      const dataUrl = cache.get(path)!;
      for (const item of items) item.resolve(dataUrl);
      continue;
    }
    // 已有并发请求 — 复用同一 Promise
    const infl = inflight.get(path);
    if (infl) {
      for (const item of items) infl.then(item.resolve, item.reject);
      continue;
    }
    toFetch.push(path);
  }

  if (toFetch.length === 0) return;

  const batchPromise = fetchLcuAssets(toFetch);

  for (const path of toFetch) {
    const items = batch.get(path)!;
    const p = batchPromise.then((results) => {
      const item = results.find((r) => r.path === path);
      if (item?.data_url) return item.data_url;
      throw new Error(item?.error ?? `资源加载失败: ${path}`);
    });

    inflight.set(path, p);
    p.then(
      (dataUrl) => {
        cacheSet(path, dataUrl);
        for (const item of items) item.resolve(dataUrl);
      },
      (err) => {
        for (const item of items) item.reject(err);
      },
    ).finally(() => {
      inflight.delete(path);
    });
  }
}

/**
 * 将 LCU 资源路径排队进入同一渲染批次的合并请求。
 * 命中缓存时立即返回；已有并发请求时复用，不会重复发起。
 */
function enqueueFetch(path: string): Promise<string> {
  const cached = cache.get(path);
  if (cached !== undefined) return Promise.resolve(cached);

  const infl = inflight.get(path);
  if (infl) return infl;

  return new Promise<string>((resolve, reject) => {
    const items = pendingPaths.get(path);
    if (items) {
      items.push({ resolve, reject });
    } else {
      pendingPaths.set(path, [{ resolve, reject }]);
    }
    scheduleFlush();
  });
}

/**
 * 包装带重试机制的 LCU 资源获取方法，处理客户端初始启动时的暂时不可达问题。
 * 重试单位仍是单个路径，语义与逐路径调用保持一致。
 */
function fetchLcuAssetWithRetry(
  path: string,
  retries = 3,
  delay = 800,
): Promise<string> {
  return enqueueFetch(path).catch((err) => {
    const errStr = String(err || "");
    // 如果是 400 Bad Request、404 Not Found 等明确缺失资源的错误，不浪费时间重试
    const isPermanentError =
      errStr.includes("400") ||
      errStr.includes("404") ||
      errStr.includes("Bad Request") ||
      errStr.includes("Not Found") ||
      errStr.includes("不允许的资源路径");

    if (retries > 0 && !isPermanentError) {
      console.warn(
        `[LcuImage] 资源加载临时异常，将在 ${delay}ms 后重试 (剩余 ${retries} 次):`,
        path,
        err,
      );
      return new Promise<void>((resolve) => setTimeout(resolve, delay)).then(
        () => fetchLcuAssetWithRetry(path, retries - 1, delay * 1.5),
      );
    }
    throw err;
  });
}

/**
 * 将 LCU 资源路径转为可用的 data URL。
 * 自动缓存，相同路径只请求一次；同一渲染批次内的多个路径合并为一次 IPC 调用。
 * @param fallbackSrcRef 资源永久加载失败（如 LCU 已移除旧图标）时使用的兜底路径
 */
export function useLcuAsset(
  pathRef: Ref<string | undefined>,
  fallbackSrcRef?: Ref<string | undefined>,
  enabledRef?: Ref<boolean>,
) {
  const src = ref("");
  const loading = ref(false);

  // 未传 enabledRef 时默认始终加载；传入时用于实现视口懒加载等延迟加载
  const isEnabled = enabledRef ?? ref(true);

  watch(
    [pathRef, isEnabled] as [Ref<string | undefined>, Ref<boolean>],
    ([path]) => {
      if (!isEnabled.value) {
        src.value = "";
        loading.value = false;
        return;
      }
      if (!path) {
        src.value = "";
        return;
      }

      // 本地静态资源（如 /images/default-avatar.svg）不走 LCU 加载，直接使用
      if (!isLcuAssetPath(path)) {
        src.value = path;
        return;
      }

      // 缓存命中 — 直接赋值
      if (cache.has(path)) {
        src.value = cache.get(path)!;
        return;
      }

      loading.value = true;

      fetchLcuAssetWithRetry(path).then(
        (dataUrl) => {
          // 仅当 pathRef 未变化时才写入（防止竞态）
          if (pathRef.value === path) {
            src.value = dataUrl;
          }
        },
        (err) => {
          const hasFallback = Boolean(fallbackSrcRef?.value);
          if (!hasFallback) {
            console.warn("[LcuImage] 资源最终加载失败:", path, err);
          }
          if (pathRef.value === path) {
            src.value = fallbackSrcRef?.value ?? "";
          }
        },
      ).finally(() => {
        loading.value = false;
      });
    },
    { immediate: true },
  );

  return { src, loading };
}
