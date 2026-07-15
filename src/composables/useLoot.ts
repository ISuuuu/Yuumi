import { ref, computed, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useToast } from "./useToast";
import { useI18n } from "vue-i18n";
import {
  fetchOpenableLoots,
  batchOpenLoots,
  smartOpenAllLoots,
  fetchLootInventory,
  disenchantLoot,
  upgradeLoot,
  rerollLoot,
  fetchEssenceBalances,
} from "../api/loot";
import type {
  OpenableLoot,
  LootProgressEvent,
  OpenBatchItem,
  LootItem,
  DisenchantItem,
  ActionProgressEvent,
} from "../api/loot";

export function useLoot() {
  const { showToast } = useToast();
  const { t } = useI18n();

  // ─── 战利品开启状态 ───
  const openableLoots = ref<OpenableLoot[]>([]);
  const lootLoading = ref(false);
  const lootError = ref<string | null>(null);
  const lootFetched = ref(false);
  const selectedLoot = ref<OpenableLoot | null>(null);
  const openQuantity = ref(1);
  const isOpening = ref(false);
  const openProgress = ref(0);
  const openTotal = ref(0);
  const openResults = ref<LootProgressEvent[]>([]);
  const showOpenPanel = ref(false);
  let unlistenProgress: (() => void) | null = null;

  // ─── 碎片库房状态 ───
  const rawLootInventory = ref<LootItem[]>([]);
  const isInventoryLoading = ref(false);
  const inventoryError = ref<string | null>(null);
  const selectedLootIds = ref<string[]>([]);
  const filterType = ref("CHAMPION");
  const filterOwned = ref("ALL");
  const filterValueType = ref<"value" | "disenchantValue">("disenchantValue");
  const filterOperator = ref<"<=" | ">=" | "=">("<=");
  const filterMaxValue = ref<number | null>(null);

  // 精粹余额
  const blueEssenceCount = ref(0);
  const orangeEssenceCount = ref(0);

  // 确认弹窗状态
  const showConfirmModal = ref(false);
  const confirmModalConfig = ref({
    title: "",
    message: "",
    confirmText: "确定",
    cancelText: "取消",
    onConfirm: () => {},
    type: "warning" as "warning" | "info" | "error" | "primary",
    details: null as { label: string; value: string; class?: string }[] | null,
  });

  // 操作进度状态
  const actionProgressTitle = ref("");
  let actionUnlisten: (() => void) | null = null;

  function executeConfirmedAction() {
    showConfirmModal.value = false;
    confirmModalConfig.value.onConfirm();
  }

  // ─── 辅助函数 ───

  function getLootDisplayName(loot: OpenableLoot): string {
    const name = loot.name;
    const id = loot.lootId;
    if (id === "CHEST_promotion") return "宝箱";
    if (id === "CHEST_champion_mastery" || id === "CHEST_generic" || id === "CHEST_hextech") return "海克斯科技宝箱";
    if (id === "CHEST_premium") return "杰作宝箱";
    if (id === "MATERIAL_key_fragment") return "钥匙碎片";
    if (id === "MATERIAL_key") return "海克斯科技钥匙";
    if (id === "MATERIAL_key_premium") return "杰作钥匙";
    if (name === id) {
      if (id.includes("ORB") || id.includes("orb")) return "法球";
      if (id.includes("CAPSULE")) return "引擎/胶囊";
    }
    return name;
  }

  function isKeyFragmentLoot(loot: OpenableLoot): boolean {
    return loot.lootId === "MATERIAL_key_fragment";
  }

  function getFriendlyNameById(lootId: string): string {
    const found = rawLootInventory.value.find(i => i.lootId === lootId);
    return found?.itemDesc ?? lootId;
  }

  // ─── 排序与计算属性 ───

  const sortedOpenableLoots = computed(() => {
    return [...openableLoots.value].sort((a, b) => {
      const getWeight = (loot: OpenableLoot) => {
        const id = loot.lootId;
        if (id === "CHEST_promotion") return 10;
        if (id === "CHEST_champion_mastery" || id === "CHEST_generic" || id === "CHEST_hextech") return 20;
        if (loot.needKey) return 30;
        return 40;
      };
      return getWeight(a) - getWeight(b);
    });
  });

  const keyNeededLoots = computed(() =>
    openableLoots.value
      .filter((l) => l.needKey && l.count > 0)
      .sort(
        (a, b) =>
          lootPriorityIndex(a.lootId) - lootPriorityIndex(b.lootId) ||
          b.name.localeCompare(a.name),
      ),
  );

  const noKeyLoots = computed(() =>
    openableLoots.value.filter((l) => !l.needKey && l.count > 0),
  );

  const totalKeyCount = computed(() => {
    const hextechKey = rawLootInventory.value.find(item => item.lootId === "MATERIAL_key");
    const premiumKey = rawLootInventory.value.find(item => item.lootId === "MATERIAL_key_premium");
    return (hextechKey?.count ?? 0) + (premiumKey?.count ?? 0);
  });

  function lootPriorityIndex(lootId: string): number {
    if (lootId === "CHEST_promotion") return 0;
    if (lootId === "CHEST_champion_mastery" || lootId === "CHEST_generic" || lootId === "CHEST_hextech") return 1;
    if (lootId.includes("ORB") || lootId.includes("orb")) return 2;
    if (lootId.includes("CAPSULE")) return 3;
    return 4;
  }

  // ─── 碎片过滤 ───

  const filteredInventory = computed(() => {
    return rawLootInventory.value.filter(item => {
      if (filterType.value === "ALL") {
        const cat = item.displayCategories.toUpperCase();
        const lootId = item.lootId.toUpperCase();
        const type = item.lootType.toUpperCase();
        if (cat === "CURRENCY" || lootId.startsWith("CURRENCY")) return false;
        if (lootId.startsWith("MATERIAL_KEY") && !lootId.includes("FRAGMENT")) return false;
        if (cat === "CHEST" || type === "CHEST" || lootId.startsWith("CHEST_")) return false;
      }
      if (filterType.value !== "ALL") {
        if (filterType.value === "ETERNAL") {
          if (!item.lootType.toUpperCase().includes("STATSTONE")) return false;
        } else if (filterType.value === "MATERIAL") {
          const cat = item.displayCategories.toUpperCase();
          if (cat !== "MATERIAL" && cat !== "CHEST" && cat !== "ORB" && cat !== "CURRENCY") return false;
        } else {
          if (item.displayCategories !== filterType.value) return false;
        }
      }
      if (filterOwned.value === "OWNED" && item.itemStatus !== "OWNED") return false;
      if (filterOwned.value === "NOT_OWNED" && item.itemStatus === "OWNED") return false;
      if (filterMaxValue.value !== null) {
        const cmpValue = filterValueType.value === "value" ? item.value : item.disenchantValue;
        const threshold = filterMaxValue.value;
        if (filterOperator.value === "<=" && cmpValue > threshold) return false;
        if (filterOperator.value === ">=" && cmpValue < threshold) return false;
        if (filterOperator.value === "=" && cmpValue !== threshold) return false;
      }
      return true;
    }).sort((a, b) => a.disenchantValue - b.disenchantValue);
  });

  const selectedLootObjects = computed(() => {
    return rawLootInventory.value.filter(item => selectedLootIds.value.includes(item.lootId));
  });

  const canUpgrade = computed(() => {
    if (selectedLootIds.value.length === 0) return false;
    return selectedLootObjects.value.some(item => {
      const cat = item.displayCategories.toUpperCase();
      return item.itemStatus !== "OWNED" && cat !== "MATERIAL" && cat !== "CHEST" && cat !== "ORB" && cat !== "CURRENCY";
    });
  });

  const upgradeBtnText = computed(() => {
    if (selectedLootIds.value.length === 0) return t("tools.lootManager.upgradeBtn") || "升级 / 解锁";
    const hasUpgrade = selectedLootObjects.value.some(item => item.upgradeEssenceCost > 0);
    const hasUnlock = selectedLootObjects.value.some(item => item.upgradeEssenceCost === 0);
    if (hasUpgrade && hasUnlock) return "升级 / 解锁";
    if (hasUnlock) return "解锁永久";
    return "升级永久";
  });

  const canReroll = computed(() => {
    if (selectedLootIds.value.length === 0) return false;
    return selectedLootObjects.value.some(item => {
      const cat = item.displayCategories.toUpperCase();
      return !item.lootType.toUpperCase().includes("STATSTONE") &&
             cat !== "MATERIAL" && cat !== "CHEST" && cat !== "ORB" && cat !== "CURRENCY";
    });
  });

  const gainBlueEssence = computed(() => {
    return selectedLootObjects.value
      .filter(item => item.displayCategories === "CHAMPION")
      .reduce((sum, item) => sum + item.disenchantValue * item.count, 0);
  });

  const gainOrangeEssence = computed(() => {
    return selectedLootObjects.value
      .filter(item => item.displayCategories !== "CHAMPION")
      .reduce((sum, item) => sum + item.disenchantValue * item.count, 0);
  });

  const progressPanelTitle = computed(() => {
    return actionProgressTitle.value || t("tools.lootOpener.opening");
  });

  const openPercentage = computed(() => {
    if (openTotal.value <= 0) return 0;
    return Math.round((openProgress.value / openTotal.value) * 100);
  });

  const maxOpenQuantity = computed(() => {
    if (!selectedLoot.value) return 0;
    const loot = selectedLoot.value;
    if (isKeyFragmentLoot(loot)) return Math.floor(loot.count / 3);
    if (!loot.needKey) return loot.count;
    return Math.min(loot.count, loot.keyCount ?? 0);
  });

  const keyDisplayName = computed(() => {
    const keyId = selectedLoot.value?.keyLootId;
    if (!keyId) return "";
    if (keyId === "MATERIAL_key") return t("tools.lootOpener.hextechKey") || "海克斯科技钥匙";
    if (keyId === "MATERIAL_key_premium") return t("tools.lootOpener.masterworkKey") || "杰作钥匙";
    return selectedLoot.value?.keyName || keyId;
  });

  const currentKeyCount = computed(() => {
    return selectedLoot.value?.keyCount ?? 0;
  });

  // ─── 核心方法 ───

  async function loadLootData() {
    lootLoading.value = true;
    lootError.value = null;
    try {
      openableLoots.value = await fetchOpenableLoots();
      lootFetched.value = true;
    } catch (e: any) {
      lootError.value = e.toString();
      console.error("获取战利品列表失败:", e);
    } finally {
      lootLoading.value = false;
    }
  }

  async function loadLootInventory() {
    isInventoryLoading.value = true;
    inventoryError.value = null;
    try {
      rawLootInventory.value = await fetchLootInventory();
      await updateEssenceBalances();
    } catch (e: any) {
      inventoryError.value = t("tools.lootManager.loadFailed", { error: e.toString() });
      console.error("获取碎片库存失败:", e);
    } finally {
      isInventoryLoading.value = false;
    }
  }

  async function updateEssenceBalances() {
    try {
      const balances = await fetchEssenceBalances();
      blueEssenceCount.value = balances.blueEssence;
      orangeEssenceCount.value = balances.orangeEssence;
    } catch (e) {
      console.error("获取精粹余额失败:", e);
    }
  }

  function loadAllData() {
    loadLootData();
    loadLootInventory();
  }

  // ─── 智能开启 ───

  function buildSmartOpenBatches(): OpenBatchItem[] {
    const batches: OpenBatchItem[] = [];
    const fragmentLoot = openableLoots.value.find(isKeyFragmentLoot);
    let forgedKeys = 0;
    if (fragmentLoot && fragmentLoot.count >= 3) {
      forgedKeys = Math.floor(fragmentLoot.count / 3);
      batches.push({
        lootId: fragmentLoot.lootId,
        name: fragmentLoot.name,
        count: forgedKeys,
        recipeName: fragmentLoot.recipeName,
        ingredients: [fragmentLoot.lootId],
      });
    }

    let remainingKeys = totalKeyCount.value + forgedKeys;

    for (const loot of noKeyLoots.value) {
      if (isKeyFragmentLoot(loot)) continue;
      batches.push({
        lootId: loot.lootId,
        name: loot.name,
        count: loot.count,
        recipeName: loot.recipeName,
        ingredients: [loot.lootId],
      });
    }

    for (const loot of keyNeededLoots.value) {
      if (isKeyFragmentLoot(loot)) continue;
      if (remainingKeys <= 0) break;
      const canOpen = Math.min(loot.count, remainingKeys);
      if (canOpen <= 0) continue;
      const ingredients = [loot.lootId];
      if (loot.keyLootId) ingredients.push(loot.keyLootId);
      batches.push({
        lootId: loot.lootId,
        name: loot.name,
        count: canOpen,
        recipeName: loot.recipeName,
        ingredients,
      });
      remainingKeys -= canOpen;
    }

    return batches;
  }

  async function handleSmartOpenAll(refreshCallback?: () => void) {
    const batches = buildSmartOpenBatches();
    const totalCount = batches.reduce((s, b) => s + b.count, 0);

    if (totalCount <= 0) {
      showToast(t("tools.lootOpener.noLootFound"), "error");
      return;
    }

    if (unlistenProgress) unlistenProgress();
    unlistenProgress = await listen<LootProgressEvent>(
      "loot-open-progress",
      (event) => {
        const evt = event.payload;
        openProgress.value = evt.current;
        openTotal.value = evt.total;
        openResults.value.push(evt);
        if (evt.current === evt.total) {
          isOpening.value = false;
          loadLootData();
          loadLootInventory();
          if (refreshCallback) refreshCallback();
        }
      },
    );

    showOpenPanel.value = true;
    isOpening.value = true;
    openProgress.value = 0;
    openTotal.value = totalCount;
    openResults.value = [];

    try {
      await smartOpenAllLoots(batches);
    } catch (e: any) {
      isOpening.value = false;
      showToast(t("tools.lootOpener.openFailed", { error: e.toString() }), "error");
    }
  }

  // ─── 单个开启 ───

  function openLootModal(loot: OpenableLoot) {
    selectedLoot.value = loot;
    let maxQ = 0;
    if (isKeyFragmentLoot(loot)) {
      maxQ = Math.floor(loot.count / 3);
    } else {
      maxQ = loot.needKey
        ? Math.min(loot.count, loot.keyCount ?? 0)
        : loot.count;
    }
    openQuantity.value = Math.max(1, maxQ);
  }

  function closeLootModal() {
    selectedLoot.value = null;
  }

  async function handleBatchOpen(refreshCallback?: () => void) {
    if (!selectedLoot.value || openQuantity.value <= 0) return;
    const loot = selectedLoot.value;
    const isFragment = isKeyFragmentLoot(loot);

    if (!isFragment && loot.needKey && openQuantity.value > maxOpenQuantity.value) {
      showToast(t("tools.lootOpener.insufficientKeys"), "error");
      return;
    }
    if (isFragment && openQuantity.value > maxOpenQuantity.value) {
      showToast(t("tools.lootOpener.insufficientFragments"), "error");
      return;
    }

    const ingredients = [loot.lootId];
    if (!isFragment && loot.keyLootId && loot.needKey) {
      ingredients.push(loot.keyLootId);
    }

    if (unlistenProgress) unlistenProgress();
    unlistenProgress = await listen<LootProgressEvent>(
      "loot-open-progress",
      (event) => {
        const evt = event.payload;
        openProgress.value = evt.current;
        openTotal.value = evt.total;
        openResults.value.push(evt);
        if (evt.current === evt.total) {
          isOpening.value = false;
          loadLootData();
          loadLootInventory();
          if (refreshCallback) refreshCallback();
        }
      },
    );

    actionProgressTitle.value = isFragment ? "正在合成钥匙..." : t("tools.lootOpener.opening");
    showOpenPanel.value = true;
    isOpening.value = true;
    openProgress.value = 0;
    openTotal.value = openQuantity.value;
    openResults.value = [];
    selectedLoot.value = null;

    try {
      await batchOpenLoots(loot.recipeName, ingredients, openQuantity.value);
    } catch (e: any) {
      isOpening.value = false;
      const errorMsg = isFragment
        ? `合成失败: ${e.toString()}`
        : t("tools.lootOpener.openFailed", { error: e.toString() });
      showToast(errorMsg, "error");
    }
  }

  function closeOpenPanel() {
    showOpenPanel.value = false;
    isOpening.value = false;
    openProgress.value = 0;
    openTotal.value = 0;
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  }

  // ─── 选择操作 ───

  function handleSelectAllFiltered() {
    selectedLootIds.value = filteredInventory.value.map(item => item.lootId);
  }

  function handleClearSelection() {
    selectedLootIds.value = [];
  }

  function toggleSelectItem(lootId: string) {
    const idx = selectedLootIds.value.indexOf(lootId);
    if (idx > -1) {
      selectedLootIds.value.splice(idx, 1);
    } else {
      selectedLootIds.value.push(lootId);
    }
  }

  // 清除操作监听
  function cleanupActionListen() {
    if (actionUnlisten) {
      actionUnlisten();
      actionUnlisten = null;
    }
  }

  // ─── 一键分解 ───

  function handleBatchDisenchant() {
    if (selectedLootObjects.value.length === 0) return;
    const count = selectedLootObjects.value.length;
    confirmModalConfig.value = {
      title: t("tools.lootManager.disenchantBtn"),
      message: `您确定要分解当前选中的 ${count} 个碎片吗？此操作无法撤销。`,
      confirmText: "确定分解",
      cancelText: "取消",
      type: "primary",
      details: [
        { label: "所选碎片种类", value: `${count} 种` },
        { label: "🔷 预计蓝色精粹回报", value: `+ ${gainBlueEssence.value}`, class: "blue-essence-text" },
        { label: "🔶 预计橙色精粹回报", value: `+ ${gainOrangeEssence.value}`, class: "orange-essence-text" }
      ],
      onConfirm: () => proceedWithDisenchant(refreshCallback)
    };
    showConfirmModal.value = true;
  }

  let refreshCallback: (() => void) | undefined;

  function setRefreshCallback(cb: () => void) {
    refreshCallback = cb;
  }

  async function proceedWithDisenchant(refreshCb?: () => void) {
    const payload: DisenchantItem[] = selectedLootObjects.value.map(item => ({
      lootId: item.lootId,
      count: item.count,
    }));

    actionProgressTitle.value = t("tools.lootManager.progressDisenchanting");
    openResults.value = [];
    openProgress.value = 0;
    openTotal.value = 0;
    showOpenPanel.value = true;
    isOpening.value = true;
    if (unlistenProgress) unlistenProgress();
    unlistenProgress = await listen<ActionProgressEvent>("loot-disenchant-progress", (event) => {
      const evt = event.payload;
      openProgress.value = evt.current;
      openTotal.value = evt.total;

      const displayName = getFriendlyNameById(evt.lootName);
      if (evt.success) {
        openResults.value.push({
          current: evt.current, total: evt.total, success: true, rewardName: `${displayName}: ${evt.rewardDesc}`,
          errorMsg: null, itemName: null,
        } as unknown as LootProgressEvent);
      } else {
        openResults.value.push({
          current: evt.current, total: evt.total, success: false, rewardName: "", errorMsg: `${displayName}: ${evt.errorMsg}`,
          itemName: null,
        } as unknown as LootProgressEvent);
      }

      if (evt.current === evt.total) {
        isOpening.value = false;
        loadLootInventory();
        if (refreshCb) refreshCb();
      }
    });

    try {
      await disenchantLoot(payload);
      selectedLootIds.value = [];
    } catch (e: any) {
      isOpening.value = false;
      showToast(t("tools.lootManager.disenchantFailed", { error: e.toString() }), "error");
    }
  }

  // ─── 批量升级 ───

  function handleBatchUpgrade() {
    const unownedItems = selectedLootObjects.value.filter(item => item.itemStatus !== "OWNED");
    if (unownedItems.length === 0) {
      showToast("没有选中任何未拥有的战利品进行操作！", "error");
      return;
    }

    const missingChampionSkins = unownedItems.filter(item => {
      const cat = item.displayCategories.toUpperCase();
      return cat === "SKIN" && item.parentItemStatus !== "OWNED";
    });
    if (missingChampionSkins.length > 0) {
      const names = missingChampionSkins.map(item => item.itemDesc).join("、");
      showToast(`未拥有以下皮肤对应的英雄：[${names}]，请先在游戏内解锁或购买该英雄！`, "error");
      return;
    }

    const hasUpgrade = unownedItems.some(item => item.upgradeEssenceCost > 0);
    const hasUnlock = unownedItems.some(item => item.upgradeEssenceCost === 0);

    const count = unownedItems.length;
    let totalBlueEssence = 0;
    let totalOrangeEssence = 0;
    unownedItems.forEach(item => {
      if (item.upgradeEssenceCost > 0) {
        if (item.displayCategories === "CHAMPION") {
          totalBlueEssence += item.upgradeEssenceCost * item.count;
        } else {
          totalOrangeEssence += item.upgradeEssenceCost * item.count;
        }
      }
    });

    if (totalBlueEssence > blueEssenceCount.value) {
      showToast(`蓝色精粹不足！升级需要 🔷 ${totalBlueEssence}，当前拥有 🔷 ${blueEssenceCount.value}`, "error");
      return;
    }
    if (totalOrangeEssence > orangeEssenceCount.value) {
      showToast(`橙色精粹不足！升级需要 🔶 ${totalOrangeEssence}，当前拥有 🔶 ${orangeEssenceCount.value}`, "error");
      return;
    }

    const detailsList: { label: string; value: string }[] = [
      { label: "待处理战利品种类", value: `${count} 种` }
    ];
    if (totalBlueEssence > 0) detailsList.push({ label: "预计消耗蓝色精粹", value: `🔷 ${totalBlueEssence}` });
    if (totalOrangeEssence > 0) detailsList.push({ label: "预计消耗橙色精粹", value: `🔶 ${totalOrangeEssence}` });

    let desc = "将消耗对应精粹并解锁为永久版";
    if (hasUnlock && !hasUpgrade) desc = "将免费解锁/激活永久道具并添加到收藏";
    else if (hasUnlock && hasUpgrade) desc = "消耗精粹升级部分碎片，其余永久道具将免费解锁";
    detailsList.push({ label: "操作说明", value: desc });

    let title = "升级 / 解锁选中项";
    if (hasUnlock && !hasUpgrade) title = "解锁选中项";
    else if (hasUpgrade && !hasUnlock) title = "升级选中项";

    confirmModalConfig.value = {
      title,
      message: `您确定要将选中的 ${count} 个未拥有项目进行升级或解锁吗？`,
      confirmText: "确定执行",
      cancelText: "取消",
      type: "primary",
      details: detailsList,
      onConfirm: () => proceedWithUpgrade(unownedItems)
    };
    showConfirmModal.value = true;
  }

  async function proceedWithUpgrade(unownedItems: LootItem[]) {
    const payload: DisenchantItem[] = unownedItems.map(item => ({
      lootId: item.lootId,
      count: item.count,
      upgradeRecipeName: item.upgradeRecipeName,
    }));

    actionProgressTitle.value = "正在升级或解锁碎片...";
    openResults.value = [];
    openProgress.value = 0;
    openTotal.value = 0;
    showOpenPanel.value = true;
    isOpening.value = true;
    if (unlistenProgress) unlistenProgress();
    unlistenProgress = await listen<ActionProgressEvent>("loot-upgrade-progress", (event) => {
      const evt = event.payload;
      openProgress.value = evt.current;
      openTotal.value = evt.total;

      const displayName = getFriendlyNameById(evt.lootName);
      if (evt.success) {
        openResults.value.push({
          current: evt.current, total: evt.total, success: true, rewardName: `${displayName}: ${evt.rewardDesc}`,
          errorMsg: null, itemName: null,
        } as unknown as LootProgressEvent);
      } else {
        openResults.value.push({
          current: evt.current, total: evt.total, success: false, rewardName: "", errorMsg: `${displayName}: ${evt.errorMsg}`,
          itemName: null,
        } as unknown as LootProgressEvent);
      }

      if (evt.current === evt.total) {
        isOpening.value = false;
        loadLootInventory();
        if (refreshCallback) refreshCallback();
      }
    });

    try {
      await upgradeLoot(payload);
      selectedLootIds.value = [];
    } catch (e: any) {
      isOpening.value = false;
      showToast(`升级失败: ${e.toString()}`, "error");
    }
  }

  // ─── 三合一重随 ───

  function handleBatchReroll() {
    const grouped: Record<string, string[]> = {};
    selectedLootObjects.value
      .filter(item => !item.lootType.toUpperCase().includes("STATSTONE"))
      .forEach(item => {
        const cat = item.displayCategories;
        if (!grouped[cat]) grouped[cat] = [];
        for (let i = 0; i < item.count; i++) grouped[cat].push(item.lootId);
      });

    const finalLootIdsToReroll: string[] = [];
    let remainingCount = 0;
    for (const cat in grouped) {
      const list = grouped[cat];
      const usableLen = list.length - (list.length % 3);
      if (usableLen > 0) finalLootIdsToReroll.push(...list.slice(0, usableLen));
      remainingCount += list.length % 3;
    }

    if (finalLootIdsToReroll.length === 0) {
      showToast(t("tools.lootManager.rerollNoGroup"), "error");
      return;
    }

    const groupsCount = finalLootIdsToReroll.length / 3;
    confirmModalConfig.value = {
      title: t("tools.lootManager.rerollBtn"),
      message: `您确定要将选中的碎片进行三合一重随吗？系统将自动分类并按3个一组进行重随。`,
      confirmText: "确定重随",
      cancelText: "取消",
      type: "primary",
      details: [
        { label: "重随合成组数", value: `${groupsCount} 组` },
        { label: "消耗碎片总数", value: `${finalLootIdsToReroll.length} 个` },
        { label: "保留未配对数", value: `${remainingCount} 个` }
      ],
      onConfirm: () => proceedWithReroll(finalLootIdsToReroll)
    };
    showConfirmModal.value = true;
  }

  async function proceedWithReroll(finalLootIdsToReroll: string[]) {
    openResults.value = [];
    openProgress.value = 0;
    openTotal.value = 0;
    showOpenPanel.value = true;
    isOpening.value = true;
    actionProgressTitle.value = t("tools.lootManager.progressRerolling");

    if (unlistenProgress) unlistenProgress();
    unlistenProgress = await listen<ActionProgressEvent>("loot-reroll-progress", (event) => {
      const evt = event.payload;
      openProgress.value = evt.current;
      openTotal.value = evt.total;

      if (evt.success) {
        openResults.value.push({
          current: evt.current, total: evt.total, success: true, rewardName: `合成成功！获得: ${evt.rewardDesc}`,
          errorMsg: null, itemName: null,
        } as unknown as LootProgressEvent);
      } else {
        openResults.value.push({
          current: evt.current, total: evt.total, success: false, rewardName: "", errorMsg: evt.errorMsg ?? "未知错误",
          itemName: null,
        } as unknown as LootProgressEvent);
      }

      if (evt.current === evt.total) {
        isOpening.value = false;
        loadLootInventory();
        if (refreshCallback) refreshCallback();
      }
    });

    try {
      await rerollLoot(finalLootIdsToReroll);
      selectedLootIds.value = [];
    } catch (e: any) {
      isOpening.value = false;
      showToast(t("tools.lootManager.rerollFailed", { error: e.toString() }), "error");
    }
  }

  // ─── 清理 ───

  function cleanup() {
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
    cleanupActionListen();
  }

  // 切换碎片类型时清空选择
  watch(filterType, () => {
    handleClearSelection();
  });

  return {
    // 开启状态
    openableLoots,
    sortedOpenableLoots,
    lootLoading,
    lootError,
    lootFetched,
    selectedLoot,
    openQuantity,
    isOpening,
    openProgress,
    openTotal,
    openResults,
    showOpenPanel,
    maxOpenQuantity,
    keyDisplayName,
    currentKeyCount,
    progressPanelTitle,
    openPercentage,

    // 库房状态
    rawLootInventory,
    isInventoryLoading,
    inventoryError,
    selectedLootIds,
    filterType,
    filterOwned,
    filterValueType,
    filterOperator,
    filterMaxValue,
    filteredInventory,
    selectedLootObjects,
    canUpgrade,
    upgradeBtnText,
    canReroll,
    gainBlueEssence,
    gainOrangeEssence,

    // 精粹
    blueEssenceCount,
    orangeEssenceCount,

    // 确认弹窗
    showConfirmModal,
    confirmModalConfig,
    executeConfirmedAction,

    // 操作进度
    actionProgressTitle,

    // 方法
    loadLootData,
    loadLootInventory,
    loadAllData,
    handleSmartOpenAll,
    openLootModal,
    closeLootModal,
    handleBatchOpen,
    closeOpenPanel,
    handleSelectAllFiltered,
    handleClearSelection,
    toggleSelectItem,
    handleBatchDisenchant,
    handleBatchUpgrade,
    handleBatchReroll,
    setRefreshCallback,
    getLootDisplayName,
    isKeyFragmentLoot,
    totalKeyCount,
    cleanup,
  };
}
