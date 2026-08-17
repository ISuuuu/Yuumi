import { ref, watch, type Ref } from "vue";

// 自定义协议前缀：WebView2 只拦截 http/https 请求，wry 会把 `http://yuumi-asset.localhost/...`
// workaround 前缀还原成 `yuumi-asset://localhost/...` 后交给 Rust 协议 handler。
// 资源经由 Rust 端 yuumi-asset:// 协议返回原始字节，替代 IPC + base64，浏览器自带内存缓存。
const ASSET_PROTOCOL = "http://yuumi-asset.localhost/";

// 与 Rust 侧白名单保持一致：仅这些路径需要走 LCU 资源加载
function isLcuAssetPath(path: string): boolean {
  return (
    path.startsWith("/lol-game-data/assets/") ||
    path.startsWith("/fe/lol-loot/assets/") ||
    path.startsWith("http://") ||
    path.startsWith("https://")
  );
}

/**
 * 将 LCU 资源路径转换为 yuumi-asset 协议 URL。
 * 路径整体 percent-encode，Rust 端 percent_decode 后还原，
 * 兼容含特殊字符的完整 CDN URL（如 https://...?x=1）。
 */
function buildAssetUrl(path: string): string {
  return ASSET_PROTOCOL + encodeURIComponent(path);
}

/**
 * 将 LCU 资源路径转为 yuumi-asset 协议 URL。
 * 资源加载由 WebView2 原生发起，经 Rust 协议 handler 返回原始字节，
 * 命中磁盘缓存时零网络请求；浏览器与 HTTP 缓存头（max-age=604800）负责去重。
 */
export function useLcuAsset(
  pathRef: Ref<string | undefined>,
  enabledRef?: Ref<boolean>,
) {
  const src = ref("");

  // 未传 enabledRef 时默认始终加载；传入时用于实现视口懒加载等延迟加载
  const isEnabled = enabledRef ?? ref(true);

  watch(
    [pathRef, isEnabled] as [Ref<string | undefined>, Ref<boolean>],
    ([path]) => {
      if (!isEnabled.value) {
        src.value = "";
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

      src.value = buildAssetUrl(path);
    },
    { immediate: true },
  );

  return { src };
}