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
let observer: IntersectionObserver | null = null;

onMounted(() => {
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) {
        enabled.value = true;
        observer?.disconnect();
      }
    },
    { rootMargin: "200px" },
  );
  if (wrapperEl.value) observer.observe(wrapperEl.value);
});

onBeforeUnmount(() => observer?.disconnect());
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
