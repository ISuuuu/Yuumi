<script setup lang="ts">
import { ref, toRef, watch } from "vue";
import { onMounted, onBeforeUnmount, nextTick } from "vue";
import { useLcuAsset } from "../composables/useLcuAsset";

const props = defineProps<{
  src?: string;
  alt?: string;
  /** LCU 资源加载失败（如已被移除的旧图标）时的兜底图片路径 */
  fallbackSrc?: string;
}>();

// 视口懒加载：进入视口前不触发 LCU 资源请求，减少首屏资源加载
const enabled = ref(false);
const { src: resolvedSrc } = useLcuAsset(toRef(props, "src"), enabled);

// 实际渲染的 src：协议 URL → 重试 → fallbackSrc → 空（error 占位）
const displaySrc = ref("");
const useFallback = ref(false);
let retryCount = 0;

watch(
  resolvedSrc,
  (val) => {
    displaySrc.value = val;
    useFallback.value = false;
    retryCount = 0;
  },
  { immediate: true },
);

function handleImgError() {
  // 首次失败：重试一次协议请求（LCU 可能刚启动，Rust 端此时会走 CDN 兜底）
  if (retryCount < 1 && resolvedSrc.value && !useFallback.value) {
    retryCount += 1;
    const current = displaySrc.value;
    displaySrc.value = "";
    void nextTick(() => {
      displaySrc.value = current;
    });
    return;
  }
  // 仍有 fallbackSrc 且未使用过：切换到兜底图片
  if (!useFallback.value && props.fallbackSrc) {
    useFallback.value = true;
    displaySrc.value = props.fallbackSrc;
    return;
  }
  // 彻底失败：显示 error 占位
  displaySrc.value = "";
}

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
      v-if="displaySrc"
      :src="displaySrc"
      :alt="alt"
      class="lcu-img"
      loading="lazy"
      @error="handleImgError"
    />
    <span v-else-if="!resolvedSrc" class="lcu-img-placeholder" />
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