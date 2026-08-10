<script setup lang="ts">
import { ref, toRef } from "vue";
import { onMounted, onBeforeUnmount } from "vue";
import { useLcuAsset } from "../composables/useLcuAsset";

const props = defineProps<{
  src?: string;
  alt?: string;
  /** LCU 资源加载失败（如已被移除的旧图标）时的兜底图片路径 */
  fallbackSrc?: string;
}>();

// 视口懒加载：进入视口前不触发 LCU 资源请求，减少首屏批量 IPC 与 base64 传输
const enabled = ref(false);
const { src: resolvedSrc, loading } = useLcuAsset(
  toRef(props, "src"),
  toRef(props, "fallbackSrc"),
  enabled,
);

const wrapperEl = ref<HTMLElement | null>(null);

// 模块级共享的 IntersectionObserver：大列表（数百个 LcuImage 实例）只创建一个 Observer，
// 通过 WeakMap 分发到各自元素，避免每实例独立创建的资源浪费
type ObserverRecord = { onVisible: () => void };
let sharedObserver: IntersectionObserver | null = null;
const observedTargets = new WeakMap<HTMLElement, ObserverRecord>();

function getSharedObserver() {
  if (!sharedObserver) {
    sharedObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const target = entry.target as HTMLElement;
          const rec = observedTargets.get(target);
          if (rec && entry.isIntersecting) {
            observedTargets.delete(target);
            sharedObserver?.unobserve(target);
            rec.onVisible();
          }
        }
      },
      { rootMargin: "200px" },
    );
  }
  return sharedObserver;
}

onMounted(() => {
  if (wrapperEl.value) {
    observedTargets.set(wrapperEl.value, {
      onVisible: () => {
        enabled.value = true;
      },
    });
    getSharedObserver().observe(wrapperEl.value);
  }
});

onBeforeUnmount(() => {
  if (wrapperEl.value) {
    observedTargets.delete(wrapperEl.value);
    sharedObserver?.unobserve(wrapperEl.value);
  }
});
</script>

<template>
  <div ref="wrapperEl" class="lcu-image-wrapper">
    <img
      v-if="resolvedSrc"
      :src="resolvedSrc"
      :alt="alt"
      class="lcu-img"
      loading="lazy"
    />
    <span v-else-if="loading || !enabled" class="lcu-img-placeholder" />
    <span v-else class="lcu-img-error" />
  </div>
</template>

<style scoped>
.lcu-image-wrapper {
  display: inline-flex;
  position: relative;
  overflow: hidden;
  width: 100%;
  height: 100%;
  border-radius: inherit;
  align-items: center;
  justify-content: center;
  background: transparent;
}
.lcu-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  display: block;
}
.lcu-img-placeholder {
  display: block;
  background: #e0e0e0;
  border-radius: inherit;
  width: 100%;
  height: 100%;
}
.lcu-img-error {
  display: block;
  background: #f0f0f0;
  border-radius: inherit;
  width: 100%;
  height: 100%;
}
</style>
