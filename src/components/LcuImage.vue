<script setup lang="ts">
import { toRef, watch } from "vue";
import { useLcuAsset } from "../composables/useLcuAsset";

const props = defineProps<{
  src?: string;
  alt?: string;
}>();

const { src: resolvedSrc, loading } = useLcuAsset(toRef(props, "src"));

watch(resolvedSrc, (val: string) => {
  console.log("[LcuImage] resolvedSrc =", val ? val.slice(0, 60) + "..." : "(empty)");
}, { immediate: true });
</script>

<template>
  <img
    v-if="resolvedSrc"
    :src="resolvedSrc"
    :alt="alt"
    @load="onLoad"
    @error="onError"
  />
  <span v-else-if="loading" class="lcu-img-placeholder" />
</template>

<style scoped>
.lcu-img-placeholder {
  display: inline-block;
  background: #e0e0e0;
  border-radius: 4px;
  width: 100%;
  height: 100%;
  min-width: 16px;
  min-height: 16px;
}
</style>
