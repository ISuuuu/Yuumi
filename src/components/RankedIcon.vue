<script setup lang="ts">
import { computed } from "vue";
import { getRankedMedalIcon, getRankedLargeIcon } from "../utils/ranked";

const props = withDefaults(
  defineProps<{
    tier?: string | null;
    size?: number | string;
    large?: boolean;
    alt?: string;
  }>(),
  {
    tier: "",
    size: 16,
    large: false,
    alt: "",
  },
);

const iconSrc = computed(() => {
  if (props.large) {
    return getRankedLargeIcon(props.tier);
  }
  return getRankedMedalIcon(props.tier);
});

const formattedSize = computed(() => {
  if (typeof props.size === "number") return `${props.size}px`;
  return props.size;
});
</script>

<template>
  <span
    v-if="iconSrc"
    class="ranked-icon-wrapper"
    :style="{
      width: formattedSize,
      height: formattedSize,
    }"
  >
    <img
      :src="iconSrc"
      :alt="alt || tier || 'rank'"
      class="ranked-icon-img"
      draggable="false"
    />
  </span>
</template>

<style scoped>
.ranked-icon-wrapper {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  vertical-align: middle;
  line-height: 1;
}

.ranked-icon-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  user-select: none;
  pointer-events: none;
}
</style>
