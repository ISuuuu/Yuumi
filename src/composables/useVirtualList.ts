import { computed, type Ref } from "vue";

/**
 * 轻量虚拟列表：固定行高 + 上下缓冲，仅渲染可视区附近的行。
 *
 * 约束：
 * - 所有行高度必须一致（由 itemHeight 指定），用于精确计算总高与偏移。
 * - 由调用方持有滚动容器 DOM，滚动时同步 scrollTop；可视高度变化时同步 viewportHeight。
 *
 * 返回：
 * - containerHeight   外层占位容器样式（height = total * itemHeight）
 * - offsetY          可视区相对列表顶部的偏移，用于 transform 定位可视行
 * - visibleItems     当前应渲染的行（含 index），供 v-for 使用
 *
 * 滚动与尺寸同步由调用方负责（本模块不碰 DOM）：调用方需在滚动时写入 scrollTop /
 * viewportHeight，并在容器尺寸变化时更新 viewportHeight。
 */
export function useVirtualList(options: {
  total: Ref<number>;
  scrollTop: Ref<number>;
  viewportHeight: Ref<number>;
  itemHeight: number;
  overscan?: number;
}) {
  const { total, scrollTop, viewportHeight, itemHeight } = options;
  const overscan = options.overscan ?? 3;

  const containerHeight = computed(() => total.value * itemHeight);

  const startIndex = computed(() => {
    const base = Math.floor(scrollTop.value / itemHeight);
    return Math.max(0, base - overscan);
  });

  const endIndex = computed(() => {
    const visibleCount = Math.ceil(viewportHeight.value / itemHeight);
    const base = Math.floor(scrollTop.value / itemHeight) + visibleCount;
    return Math.min(total.value, base + overscan);
  });

  const offsetY = computed(() => startIndex.value * itemHeight);

  const visibleItems = computed(() => {
    const list: { index: number }[] = [];
    for (let i = startIndex.value; i < endIndex.value; i++) {
      list.push({ index: i });
    }
    return list;
  });

  return {
    containerHeight,
    offsetY,
    visibleItems,
  };
}
