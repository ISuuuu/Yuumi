import { inject, provide, ref, type InjectionKey, type Ref } from "vue";

/**
 * LcuQuickActionsCard 各子块共享的操作进行中状态。
 * 与拆分前行为一致：任意一个操作执行期间，所有绑定该状态的按钮统一禁用/加载。
 */
const quickActionsLoadingKey: InjectionKey<Ref<boolean>> = Symbol(
  "quick-actions-loading",
);

export function provideQuickActionsLoading(): Ref<boolean> {
  const loading = ref(false);
  provide(quickActionsLoadingKey, loading);
  return loading;
}

export function useQuickActionsLoading(): Ref<boolean> {
  return inject(quickActionsLoadingKey, ref(false));
}
