<template>
  <span
    class="position-icon-wrapper"
    :style="{
      width: formattedSize,
      height: formattedSize,
      color: resolvedColor,
    }"
  >
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class="position-svg"
      viewBox="0 0 24 24"
    >
      <!-- TOP -->
      <g v-if="normalizedKey === 'TOP'" fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path d="M19 3l-4 4H7v8l-4 4V3h16z" />
          <path
            d="M21 5l-4 4H9v8l-4 4V5h16z"
            opacity=".2"
            transform="rotate(-180 13 13)"
          />
          <path d="M10 10H14V14H10z" opacity=".2" />
        </g>
      </g>

      <!-- JUNGLE -->
      <g v-else-if="normalizedKey === 'JUNGLE'" fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path
            d="M5.14 2c1.58 1.21 5.58 5.023 6.976 9.953s0 10.047 0 10.047c-2.749-3.164-5.893-5.2-6.18-5.382l-.02-.013C5.45 13.814 3 8.79 3 8.79c3.536.867 4.93 4.279 4.93 4.279C7.558 8.698 5.14 2 5.14 2zm14.976 5.907s-1.243 2.471-1.814 4.604c-.235.878-.285 2.2-.29 3.058v.282c.003.347.01.568.01.568s-1.738 2.397-3.38 3.678c.088-1.601.062-3.435-.208-5.334.928-2.023 2.846-5.454 5.682-6.856zm-2.124-5.331s-2.325 3.052-2.836 6.029c-.11.636-.201 1.194-.284 1.695-.379.584-.73 1.166-1.05 1.733-.033-.125-.06-.25-.095-.375-.302-1.07-.704-2.095-1.16-3.08.053-.146.103-.29.17-.438 0 0 1.814-3.78 5.255-5.564z"
          />
        </g>
      </g>

      <!-- MIDDLE -->
      <g v-else-if="normalizedKey === 'MIDDLE'" fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path d="M15 3l-4 4H7v4l-4 4V3h12z" opacity=".2" />
          <path
            d="M21 9l-4 4h-4v4l-4 4V9h12z"
            opacity=".2"
            transform="rotate(-180 15 15)"
          />
          <path d="M18 3L21 3 21 6 6 21 3 21 3 18z" />
        </g>
      </g>

      <!-- BOTTOM -->
      <g v-else-if="normalizedKey === 'BOTTOM'" fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path d="M19 3l-4 4H7v8l-4 4V3h16z" opacity=".2" />
          <path
            d="M21 5l-4 4H9v8l-4 4V5h16z"
            transform="rotate(-180 13 13)"
          />
          <path d="M10 10H14V14H10z" opacity=".2" />
        </g>
      </g>

      <!-- UTILITY / SUPPORT -->
      <g v-else-if="normalizedKey === 'UTILITY'" fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path
            d="M12.833 10.833L14.5 17.53v.804L12.833 20h-1.666L9.5 18.333v-.804l1.667-6.696h1.666zM7 7.5L9.5 10l-1.667 4.167-2.5-2.5L6.167 10h-2.5L2 7.5h5zm15 0L20.333 10h-2.5l.834 1.667-2.5 2.5L14.5 10 17 7.5h5zM13.743 5l.757.833v.834l-1.667 2.5h-1.666L9.5 6.667v-.834L10.257 5h3.486z"
          />
        </g>
      </g>

      <!-- ALL / DEFAULT -->
      <g v-else fill="none" fill-rule="evenodd">
        <g fill="currentColor" fill-rule="nonzero">
          <path
            d="M16.293 17.03c.362.628.147 1.43-.48 1.793-.629.364-1.431.149-1.794-.479l-2.144-3.717-2.144 3.717c-.363.628-1.165.843-1.793.48-.628-.363-.843-1.166-.48-1.793l2.144-3.718h-4.29c-.724 0-1.312-.587-1.312-1.312 0-.727.588-1.314 1.313-1.314h4.289L7.457 6.969c-.362-.627-.147-1.43.48-1.792.629-.364 1.431-.149 1.794.479l2.144 3.717 2.144-3.717c.363-.628 1.165-.843 1.793-.48.628.363.843 1.166.48 1.793l-2.144 3.718h4.29c.725 0 1.312.587 1.312 1.312 0 .727-.587 1.314-1.312 1.314h-4.29l2.145 3.718z"
          />
        </g>
      </g>
    </svg>
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  normalizePosition,
  POSITION_COLORS,
  type PositionKey,
} from "../utils/matchAnalysis";

const props = withDefaults(
  defineProps<{
    position?: string | null;
    size?: number | string;
    color?: string;
    usePositionColor?: boolean;
  }>(),
  {
    position: "ALL",
    size: 16,
    color: "",
    usePositionColor: false,
  },
);

const normalizedKey = computed<PositionKey | "ALL">(() => {
  const norm = normalizePosition(props.position);
  return norm ?? "ALL";
});

const formattedSize = computed(() => {
  if (typeof props.size === "number") {
    return `${props.size}px`;
  }
  return props.size;
});

const resolvedColor = computed(() => {
  if (props.color) return props.color;
  if (props.usePositionColor) {
    if (normalizedKey.value !== "ALL") {
      return POSITION_COLORS[normalizedKey.value];
    }
    return "#8293a7";
  }
  return "currentColor";
});
</script>

<style scoped>
.position-icon-wrapper {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  vertical-align: middle;
  line-height: 1;
  flex-shrink: 0;
}

.position-svg {
  width: 100%;
  height: 100%;
  display: block;
}
</style>
