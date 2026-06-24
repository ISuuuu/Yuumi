<script setup lang="ts">
import { toRef } from "vue";
import { useLcuAsset } from "../composables/useLcuAsset";

const props = defineProps<{
  src?: string;
  alt?: string;
}>();

const { src: resolvedSrc, loading } = useLcuAsset(toRef(props, "src"));
</script>

<template>
  <div class="lcu-image-wrapper">
    <img v-if="resolvedSrc" :src="resolvedSrc" :alt="alt" class="lcu-img" />
    <span v-else-if="loading" class="lcu-img-placeholder" />
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
