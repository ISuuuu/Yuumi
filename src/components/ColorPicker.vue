<template>
  <div class="cp-wrapper" ref="rootRef">
    <!-- 触发器：和原生 color input 一样的色块 -->
    <div class="cp-swatch" :style="swatchStyle" @click="open = !open">
      <span class="cp-alpha-tag" v-if="alphaPercent < 100"
        >{{ alphaPercent }}%</span
      >
    </div>

    <!-- 弹出面板 -->
    <div v-if="open" class="cp-panel">
      <!-- 色相/饱和度选择区 -->
      <div
        class="cp-sv"
        ref="svRef"
        :style="{ backgroundColor: hueColor }"
        @mousedown="onSvDown"
      >
        <div class="cp-sv-pointer" :style="svPointerStyle"></div>
      </div>

      <!-- 色相滑条 -->
      <div class="cp-hue" ref="hueRef" @mousedown="onHueDown">
        <div class="cp-hue-pointer" :style="huePointerStyle"></div>
      </div>

      <!-- Hex 输入行 -->
      <div class="cp-row cp-row-hex">
        <label class="cp-row-label">十六进制</label>
        <div class="cp-input-wrap">
          <span class="cp-input-prefix">#</span>
          <input
            type="text"
            class="cp-hex-input"
            :value="hexNoAlpha"
            maxlength="6"
            @change="onHexInput(($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>

      <!-- RGB 三原色行 -->
      <div class="cp-row cp-row-rgb">
        <label class="cp-row-label">RGB</label>
        <div class="cp-rgb-group">
          <div class="cp-rgb-item">
            <span class="cp-rgb-letter cp-r">R</span>
            <input
              type="number"
              min="0"
              max="255"
              :value="rgb.r"
              @change="
                onRgbInput('r', ($event.target as HTMLInputElement).value)
              "
            />
          </div>
          <div class="cp-rgb-item">
            <span class="cp-rgb-letter cp-g">G</span>
            <input
              type="number"
              min="0"
              max="255"
              :value="rgb.g"
              @change="
                onRgbInput('g', ($event.target as HTMLInputElement).value)
              "
            />
          </div>
          <div class="cp-rgb-item">
            <span class="cp-rgb-letter cp-b">B</span>
            <input
              type="number"
              min="0"
              max="255"
              :value="rgb.b"
              @change="
                onRgbInput('b', ($event.target as HTMLInputElement).value)
              "
            />
          </div>
        </div>
      </div>

      <!-- 透明度行 -->
      <div class="cp-row cp-row-alpha">
        <label class="cp-row-label">透明度</label>
        <div class="cp-input-wrap">
          <input
            type="number"
            min="0"
            max="100"
            :value="alphaPercent"
            @change="onAlphaInput(($event.target as HTMLInputElement).value)"
          />
          <span class="cp-input-suffix">%</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
  (e: "change", v: string): void;
}>();

const open = ref(false);
const rootRef = ref<HTMLElement | null>(null);
const svRef = ref<HTMLElement | null>(null);
const hueRef = ref<HTMLElement | null>(null);

// ---- color state ----
const h = ref(0);
const s = ref(1);
const v = ref(1);
const a = ref(1);

// ---- hex <-> rgb <-> hsv ----
function parseHex(hex: string): { r: number; g: number; b: number; a: number } {
  let str = hex.replace("#", "");
  let alpha = 255;
  if (str.length === 8) {
    alpha = parseInt(str.slice(0, 2), 16);
    str = str.slice(2);
  }
  if (str.length === 3)
    str = str
      .split("")
      .map((c) => c + c)
      .join("");
  return {
    r: parseInt(str.slice(0, 2), 16) || 0,
    g: parseInt(str.slice(2, 4), 16) || 0,
    b: parseInt(str.slice(4, 6), 16) || 0,
    a: alpha,
  };
}

function toHex8(r: number, g: number, b: number, alpha: number): string {
  const h2 = (n: number) =>
    Math.max(0, Math.min(255, Math.round(n)))
      .toString(16)
      .padStart(2, "0");
  return `#${h2(alpha)}${h2(r)}${h2(g)}${h2(b)}`.toUpperCase();
}

function rgbToHsv(
  r: number,
  g: number,
  b: number,
): { h: number; s: number; v: number } {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b);
  const d = max - min;
  let hue = 0;
  if (d > 0) {
    if (max === r) hue = ((g - b) / d) % 6;
    else if (max === g) hue = (b - r) / d + 2;
    else hue = (r - g) / d + 4;
    hue *= 60;
    if (hue < 0) hue += 360;
  }
  return { h: hue, s: max === 0 ? 0 : d / max, v: max };
}

function hsvToRgb(
  hue: number,
  sat: number,
  val: number,
): { r: number; g: number; b: number } {
  const c = val * sat;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = val - c;
  let r = 0,
    g = 0,
    b = 0;
  if (hue < 60) {
    r = c;
    g = x;
  } else if (hue < 120) {
    r = x;
    g = c;
  } else if (hue < 180) {
    g = c;
    b = x;
  } else if (hue < 240) {
    g = x;
    b = c;
  } else if (hue < 300) {
    r = x;
    b = c;
  } else {
    r = c;
    b = x;
  }
  return {
    r: Math.round((r + m) * 255),
    g: Math.round((g + m) * 255),
    b: Math.round((b + m) * 255),
  };
}

// sync from prop
watch(
  () => props.modelValue,
  (val) => {
    if (!val) return;
    const { r, g, b, a: alpha } = parseHex(val);
    const hsv = rgbToHsv(r, g, b);
    h.value = hsv.h;
    s.value = hsv.s;
    v.value = hsv.v;
    a.value = alpha / 255;
  },
  { immediate: true },
);

const rgb = computed(() => hsvToRgb(h.value, s.value, v.value));
const hueColor = computed(() => {
  const c = hsvToRgb(h.value, 1, 1);
  return `rgb(${c.r}, ${c.g}, ${c.b})`;
});
const hexNoAlpha = computed(() => {
  const c = rgb.value;
  const h2 = (n: number) => n.toString(16).padStart(2, "0");
  return `${h2(c.r)}${h2(c.g)}${h2(c.b)}`.toUpperCase();
});
const alphaPercent = computed(() => Math.round(a.value * 100));

const swatchStyle = computed(() => {
  const c = rgb.value;
  return { backgroundColor: `rgb(${c.r}, ${c.g}, ${c.b})` };
});

const svPointerStyle = computed(() => ({
  left: `${s.value * 100}%`,
  top: `${(1 - v.value) * 100}%`,
}));
const huePointerStyle = computed(() => ({
  left: `${(h.value / 360) * 100}%`,
}));

function emitChange() {
  const c = rgb.value;
  const alpha = Math.round(a.value * 255);
  const hex = toHex8(c.r, c.g, c.b, alpha);
  emit("update:modelValue", hex);
  emit("change", hex);
}

// ---- SV dragging ----
function updateSv(e: MouseEvent | Touch) {
  const rect = svRef.value?.getBoundingClientRect();
  if (!rect) return;
  s.value = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  v.value = Math.max(0, Math.min(1, 1 - (e.clientY - rect.top) / rect.height));
}
function onSvDown(e: MouseEvent) {
  updateSv(e);
  const move = (ev: MouseEvent) => updateSv(ev);
  const up = () => {
    document.removeEventListener("mousemove", move);
    document.removeEventListener("mouseup", up);
    emitChange();
  };
  document.addEventListener("mousemove", move);
  document.addEventListener("mouseup", up);
}

// ---- Hue dragging ----
function updateHue(e: MouseEvent | Touch) {
  const rect = hueRef.value?.getBoundingClientRect();
  if (!rect) return;
  h.value = Math.max(
    0,
    Math.min(360, ((e.clientX - rect.left) / rect.width) * 360),
  );
}
function onHueDown(e: MouseEvent) {
  updateHue(e);
  const move = (ev: MouseEvent) => updateHue(ev);
  const up = () => {
    document.removeEventListener("mousemove", move);
    document.removeEventListener("mouseup", up);
    emitChange();
  };
  document.addEventListener("mousemove", move);
  document.addEventListener("mouseup", up);
}

// ---- input handlers ----
function onHexInput(val: string) {
  const clean = val.replace(/[^0-9a-fA-F]/g, "").slice(0, 6);
  if (clean.length === 6) {
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    const hsv = rgbToHsv(r, g, b);
    h.value = hsv.h;
    s.value = hsv.s;
    v.value = hsv.v;
    emitChange();
  }
}
function onRgbInput(channel: "r" | "g" | "b", val: string) {
  const n = Math.max(0, Math.min(255, parseInt(val) || 0));
  const cur = rgb.value;
  const r = channel === "r" ? n : cur.r;
  const g = channel === "g" ? n : cur.g;
  const b = channel === "b" ? n : cur.b;
  const hsv = rgbToHsv(r, g, b);
  h.value = hsv.h;
  s.value = hsv.s;
  v.value = hsv.v;
  emitChange();
}
function onAlphaInput(val: string) {
  const n = Math.max(0, Math.min(100, parseInt(val) || 0));
  a.value = n / 100;
  emitChange();
}

// ---- outside click ----
function onOutside(e: MouseEvent) {
  if (rootRef.value && !rootRef.value.contains(e.target as Node)) {
    if (open.value) emitChange();
    open.value = false;
  }
}
onMounted(() => document.addEventListener("mousedown", onOutside));
onUnmounted(() => document.removeEventListener("mousedown", onOutside));
</script>

<style scoped>
.cp-wrapper {
  position: relative;
  display: inline-block;
}

.cp-swatch {
  width: 36px;
  height: 24px;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  cursor: pointer;
  position: relative;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}
.cp-alpha-tag {
  font-size: 0.6rem;
  color: #fff;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.6);
  pointer-events: none;
  font-weight: 600;
}

.cp-panel {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 1000;
  width: 240px;
  padding: 12px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.14),
    0 2px 6px rgba(0, 0, 0, 0.06);
}

.cp-sv {
  position: relative;
  width: 100%;
  height: 130px;
  border-radius: 6px;
  cursor: crosshair;
  background-image:
    linear-gradient(to top, #000, transparent),
    linear-gradient(to right, #fff, transparent);
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.05);
}
.cp-sv-pointer {
  position: absolute;
  width: 12px;
  height: 12px;
  border: 2px solid #fff;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  box-shadow:
    0 0 4px rgba(0, 0, 0, 0.5),
    0 0 0 1px rgba(0, 0, 0, 0.15);
  pointer-events: none;
}

.cp-hue {
  position: relative;
  width: 100%;
  height: 10px;
  margin-top: 10px;
  border-radius: 5px;
  cursor: pointer;
  background: linear-gradient(
    to right,
    #f00,
    #ff0,
    #0f0,
    #0ff,
    #00f,
    #f0f,
    #f00
  );
}
.cp-hue-pointer {
  position: absolute;
  top: -3px;
  width: 16px;
  height: 16px;
  border: 2px solid #fff;
  border-radius: 50%;
  transform: translateX(-50%);
  box-shadow:
    0 0 4px rgba(0, 0, 0, 0.4),
    0 0 0 1px rgba(0, 0, 0, 0.1);
  pointer-events: none;
}

/* ---- 输入行通用 ---- */
.cp-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 10px;
}
.cp-row-label {
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--text-muted);
  width: 50px;
  flex-shrink: 0;
  user-select: none;
}

/* ---- Hex 行 ---- */
.cp-row-hex .cp-input-wrap {
  flex: 1;
  display: flex;
  align-items: center;
  height: 26px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--card-bg);
  overflow: hidden;
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
}
.cp-row-hex .cp-input-wrap:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-15);
}
.cp-input-prefix {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 100%;
  font-size: 0.72rem;
  color: var(--text-dimmed);
  background: rgba(0, 0, 0, 0.03);
  flex-shrink: 0;
}
.cp-hex-input {
  flex: 1;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-color);
  font-size: 0.72rem;
  text-align: center;
  padding: 0 4px;
  outline: none;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  letter-spacing: 1px;
  text-transform: uppercase;
}

/* ---- RGB 行 ---- */
.cp-row-rgb .cp-rgb-group {
  flex: 1;
  display: flex;
  gap: 4px;
  min-width: 0;
}
.cp-rgb-item {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  height: 24px;
  border: 1px solid var(--border-color);
  border-radius: 5px;
  background: var(--card-bg);
  overflow: hidden;
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
}
.cp-rgb-item:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-15);
}
.cp-rgb-letter {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 100%;
  font-size: 0.6rem;
  font-weight: 700;
  color: #fff;
  flex-shrink: 0;
}
.cp-r {
  background: #e74c3c;
}
.cp-g {
  background: #2ecc71;
}
.cp-b {
  background: #3498db;
}
.cp-rgb-item input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-color);
  font-size: 0.65rem;
  text-align: center;
  padding: 0 1px;
  outline: none;
  appearance: textfield;
  -moz-appearance: textfield;
}
.cp-rgb-item input::-webkit-outer-spin-button,
.cp-rgb-item input::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

/* ---- 透明度行 ---- */
.cp-row-alpha .cp-input-wrap {
  display: flex;
  align-items: center;
  height: 26px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--card-bg);
  overflow: hidden;
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
  width: 70px;
}
.cp-row-alpha .cp-input-wrap:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-15);
}
.cp-row-alpha input {
  flex: 1;
  height: 100%;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text-color);
  font-size: 0.7rem;
  text-align: center;
  padding: 0 2px;
  outline: none;
  appearance: textfield;
  -moz-appearance: textfield;
}
.cp-row-alpha input::-webkit-outer-spin-button,
.cp-row-alpha input::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
.cp-input-suffix {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 100%;
  font-size: 0.7rem;
  color: var(--text-dimmed);
  background: rgba(0, 0, 0, 0.03);
  flex-shrink: 0;
}
</style>
