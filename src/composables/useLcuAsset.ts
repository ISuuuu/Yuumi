import { ref, watch, type Ref } from "vue";
import { fetchLcuAsset } from "../api/lcu";

// LRU 缓存：Map 保持插入顺序，超过上限时淘汰最久未使用的条目
const MAX_CACHE_SIZE = 500;
const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();

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

/**
 * 包装带重试机制的 LCU 资源获取方法，处理客户端初始启动时的暂时不可达问题
 */
function fetchLcuAssetWithRetry(
  path: string,
  retries = 3,
  delay = 1000,
): Promise<string> {
  return fetchLcuAsset(path).catch((err) => {
    if (retries > 0) {
      console.warn(
        `[LcuImage] 资源加载失败，将在 ${delay}ms 后重试 (剩余 ${retries} 次):`,
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
 * 自动缓存，相同路径只请求一次。
 */
export function useLcuAsset(pathRef: Ref<string | undefined>) {
  const src = ref("");
  const loading = ref(false);

  watch(
    pathRef,
    (path) => {
      if (!path) {
        src.value = "";
        return;
      }

      // 拦截以 /fe/lol-loot/ 开头的战利品前端资源，映射至 CommunityDragon 在线 CDN
      if (path.startsWith("/fe/lol-loot/")) {
        const subPath = path.slice(13); // 移除 "/fe/lol-loot/" (长度 13)
        src.value = `https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-loot/global/default/${subPath}`;
        return;
      }

      // 缓存命中 — 直接赋值
      if (cache.has(path)) {
        src.value = cache.get(path)!;
        return;
      }

      loading.value = true;

      // 复用已有的并发请求
      if (!inflight.has(path)) {
        inflight.set(path, fetchLcuAssetWithRetry(path));
      }

      inflight
        .get(path)!
        .then(
          (dataUrl) => {
            cacheSet(path, dataUrl);
            // 仅当 pathRef 未变化时才写入（防止竞态）
            if (pathRef.value === path) {
              src.value = dataUrl;
            }
          },
          (err) => {
            console.warn("[LcuImage] 资源最终加载失败:", path, err);
            if (pathRef.value === path) {
              src.value = "";
            }
          },
        )
        .finally(() => {
          inflight.delete(path);
          loading.value = false;
        });
    },
    { immediate: true },
  );

  return { src, loading };
}
