<template>
  <div :style="$attrs.style as any" :class="$attrs.class as any">
    <n-color-picker
      :value="pureColor"
      :show-alpha="false"
      @update:value="onColorUpdate"
      @update:show="onShowUpdate"
      v-bind="filteredAttrs"
    >
      <template #action>
        <div style="display: flex; flex-direction: column; width: 100%; gap: 8px;">
          <div style="display: flex; align-items: center; gap: 8px; width: 100%;">
            <span style="font-size: 12px; color: var(--text-dimmed); white-space: nowrap;">透明度</span>
            <n-slider
              :value="alphaPercent"
              @update:value="onAlphaSliderUpdate"
              :min="0"
              :max="100"
              style="flex: 1; min-width: 60px;"
            />
            <n-input-number
              :value="alphaPercent"
              @update:value="onAlphaInputUpdate"
              :min="0"
              :max="100"
              size="small"
              style="width: 70px; flex-shrink: 0;"
              :show-button="false"
            >
              <template #suffix>%</template>
            </n-input-number>
          </div>
        </div>
      </template>
    </n-color-picker>
  </div>
</template>

<script setup lang="ts">
import { computed, useAttrs } from 'vue';

defineOptions({
  inheritAttrs: false
});

const props = defineProps<{
  value: string; // 预期是 #AARRGGBB 或类似格式，如果没有 alpha 则默认为 FF
}>();

const emit = defineEmits<{
  (e: 'update:value', val: string): void;
  (e: 'save'): void; // 当面板关闭时触发持久化保存
}>();

const attrs = useAttrs();
const filteredAttrs = computed(() => {
  const { style, class: className, ...rest } = attrs;
  return rest;
});

// 从颜色提取透明度百分比
const alphaPercent = computed(() => {
  const color = props.value;
  if (!color) return 100;
  if (color.startsWith('#') && color.length === 9) {
    const alphaHex = color.slice(1, 3);
    return Math.round((parseInt(alphaHex, 16) / 255) * 100);
  }
  return 100;
});

// 提取纯色 #RRGGBB 给颜色选择器
const pureColor = computed(() => {
  const color = props.value;
  if (!color) return '#000000';
  if (color.startsWith('#') && color.length === 9) {
    return '#' + color.slice(3); // #RRGGBB
  }
  return color;
});

// 将纯色和透明度百分比合成 #AARRGGBB
function buildColor(pureHex: string, percent: number) {
  const cleanHex6 = pureHex.startsWith('#') ? pureHex.slice(1) : pureHex;
  const alpha = Math.min(100, Math.max(0, percent)) / 100;
  const alphaHex = Math.round(alpha * 255).toString(16).padStart(2, '0');
  return `#${alphaHex}${cleanHex6}`;
}

function onColorUpdate(newPureHex: string) {
  emit('update:value', buildColor(newPureHex, alphaPercent.value));
}

function onAlphaSliderUpdate(val: number) {
  emit('update:value', buildColor(pureColor.value, val));
}

function onAlphaInputUpdate(val: number | null) {
  if (val === null) return;
  emit('update:value', buildColor(pureColor.value, val));
}

// 当面板关闭时通知外部进行写入保存，避免拖动时的高频 I/O 卡死
function onShowUpdate(show: boolean) {
  if (!show) {
    emit('save');
  }
}
</script>
