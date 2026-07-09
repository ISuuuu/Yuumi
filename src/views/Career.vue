<script setup lang="ts">
import {
  ref,
  watch,
  computed,
  inject,
  onMounted,
  onUnmounted,
  type Ref,
} from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";
import {
  fetchCurrentSummoner,
  fetchMatchHistory,
  fetchMatchHistorySgp,
  lcuRequest,
  fetchConfig,
} from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { listen } from "@tauri-apps/api/event";
import { fetchOpenableLoots, batchOpenLoots, smartOpenAllLoots, fetchLootInventory, disenchantLoot, rerollLoot, upgradeLoot, fetchEssenceBalances } from "../api/loot";
import type { OpenableLoot, LootProgressEvent, OpenBatchItem, LootItem, DisenchantItem, ActionProgressEvent } from "../api/loot";
import { useToast } from "../composables/useToast";

// 模块作用域内存缓存单例，防止频繁切换标签页触发无意义的 API 数据刷新
let cachedSummoner: SummonerDisplay | null = null;
let cachedMatches: MatchDisplay[] = [];
let cachedRecentMatches: MatchDisplay[] = [];
let cachedRankedQueues: any[] = [];
let lastFetchedTime = 0;

const store = useLcuStore();
const { t, te } = useI18n();
const { showToast } = useToast();
const currentTab = ref("matches"); // matches (生涯战绩), loot (战利品)

// ─── 战利品批量开启逻辑 ───
const openableLoots = ref<OpenableLoot[]>([]);
const lootLoading = ref(false);
const lootError = ref<string | null>(null);
const selectedLoot = ref<OpenableLoot | null>(null);
const openQuantity = ref(1);
const isOpening = ref(false);
const openProgress = ref(0);
const openTotal = ref(0);
const openResults = ref<LootProgressEvent[]>([]);
const showOpenPanel = ref(false);
let unlistenProgress: (() => void) | null = null;

function getLootDisplayName(loot: OpenableLoot): string {
  const name = loot.name;
  const id = loot.lootId;
  if (id === "CHEST_promotion") {
    return "宝箱";
  }
  if (id === "CHEST_champion_mastery" || id === "CHEST_generic" || id === "CHEST_hextech") {
    return "海克斯科技宝箱";
  }
  if (id === "CHEST_premium") {
    return "杰作宝箱";
  }
  if (name === id) {
    if (id.includes("ORB") || id.includes("orb")) return "法球";
    if (id.includes("CAPSULE")) return "引擎/胶囊";
  }
  return name;
}

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

async function loadOpenableLoots() {
  lootLoading.value = true;
  lootError.value = null;
  try {
    openableLoots.value = await fetchOpenableLoots();
    if (openableLoots.value.length === 0) {
      lootError.value = t("tools.lootOpener.noLootFound");
    }
  } catch (e: any) {
    lootError.value = e.toString();
    console.error("获取战利品列表失败:", e);
  } finally {
    lootLoading.value = false;
  }
}

// ─── 智能分配辅助 ───
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
  const keyItems = openableLoots.value.filter((l) => l.needKey);
  if (keyItems.length === 0) return 0;
  return Math.max(0, ...keyItems.map((l) => l.keyCount ?? 0));
});

function lootPriorityIndex(lootId: string): number {
  if (lootId === "CHEST_promotion") return 0;
  if (lootId === "CHEST_champion_mastery" || lootId === "CHEST_generic" || lootId === "CHEST_hextech") return 1;
  if (lootId.includes("ORB") || lootId.includes("orb")) return 2;
  if (lootId.includes("CAPSULE")) return 3;
  return 4;
}

function buildSmartOpenBatches(): OpenBatchItem[] {
  const batches: OpenBatchItem[] = [];
  let remainingKeys = totalKeyCount.value;

  for (const loot of noKeyLoots.value) {
    batches.push({
      lootId: loot.lootId,
      name: loot.name,
      count: loot.count,
      recipeName: loot.recipeName,
      ingredients: [loot.lootId],
    });
  }

  for (const loot of keyNeededLoots.value) {
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

async function handleSmartOpenAll() {
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
        loadOpenableLoots();
        loadSummoner(true); // 开启完毕后，自动刷新生涯资产
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
    showToast(
      t("tools.lootOpener.openFailed", { error: e.toString() }),
      "error",
    );
  }
}

function openLootModal(loot: OpenableLoot) {
  selectedLoot.value = loot;
  const maxQ = loot.needKey
    ? Math.min(loot.count, loot.keyCount ?? 0)
    : loot.count;
  openQuantity.value = Math.max(1, maxQ);
}

function closeLootModal() {
  selectedLoot.value = null;
}

const maxOpenQuantity = computed(() => {
  if (!selectedLoot.value) return 0;
  const loot = selectedLoot.value;
  if (!loot.needKey) return loot.count;
  return Math.min(loot.count, loot.keyCount ?? 0);
});

const keyDisplayName = computed(() => {
  const keyId = selectedLoot.value?.keyLootId;
  if (!keyId) return "";
  if (keyId === "MATERIAL_key") {
    return t("tools.lootOpener.hextechKey") || "海克斯科技钥匙";
  } else if (keyId === "MATERIAL_key_premium") {
    return t("tools.lootOpener.masterworkKey") || "杰作钥匙";
  }
  return selectedLoot.value?.keyName || keyId;
});

const currentKeyCount = computed(() => {
  return selectedLoot.value?.keyCount ?? 0;
});

async function handleBatchOpen() {
  if (!selectedLoot.value || openQuantity.value <= 0) return;
  if (openQuantity.value > maxOpenQuantity.value) {
    showToast(
      t("tools.lootOpener.insufficientKeys"),
      "error",
    );
    return;
  }

  const loot = selectedLoot.value;
  const ingredients = [loot.lootId];
  if (loot.keyLootId && loot.needKey) {
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
        loadOpenableLoots();
        loadSummoner(true); // 开启完毕后，自动刷新生涯资产
      }
    },
  );

  showOpenPanel.value = true;
  isOpening.value = true;
  openProgress.value = 0;
  openTotal.value = openQuantity.value;
  openResults.value = [];
  selectedLoot.value = null;

  try {
    await batchOpenLoots(
      loot.recipeName,
      ingredients,
      openQuantity.value,
    );
  } catch (e: any) {
    isOpening.value = false;
    showToast(
      t("tools.lootOpener.openFailed", { error: e.toString() }),
      "error",
    );
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

// ─── 碎片库存管理（分解 / 三合一重随）───

// 精粹余额
const blueEssenceCount = ref(0);
const orangeEssenceCount = ref(0);

async function updateEssenceBalances() {
  try {
    const balances = await fetchEssenceBalances();
    blueEssenceCount.value = balances.blueEssence;
    orangeEssenceCount.value = balances.orangeEssence;
  } catch (e) {
    console.error("获取精粹余额失败:", e);
  }
}

// 碎片列表
const rawLootInventory = ref<LootItem[]>([]);
const isInventoryLoading = ref(false);
const inventoryError = ref<string | null>(null);

// 筛选字段
const filterType = ref("CHAMPION"); // CHAMPION, SKIN, EMOTE, WARDSKIN, SUMMONERICON, ALL
const filterOwned = ref("ALL"); // ALL, OWNED, NOT_OWNED
const filterValueType = ref<"value" | "disenchantValue">("disenchantValue");
const filterOperator = ref<"<=" | ">=" | "=">("<=");
const filterMaxValue = ref<number | null>(null);

// 选中的碎片 IDs
const selectedLootIds = ref<string[]>([]);

// 切换碎片类型时自动清空已选列表，避免累计不同类型的选择影响批量操作
watch(filterType, () => {
  handleClearSelection();
});

// 自定义美化确认弹窗状态
const showConfirmModal = ref(false);
const confirmModalConfig = ref({
  title: "",
  message: "",
  confirmText: "确定",
  cancelText: "取消",
  onConfirm: () => {},
  type: "warning", // 'warning' | 'info' | 'error'
  details: null as { label: string; value: string; class?: string }[] | null,
});

function executeConfirmedAction() {
  showConfirmModal.value = false;
  confirmModalConfig.value.onConfirm();
}

// 操作进度状态（复用已有的 showOpenPanel/isOpening/openProgress 相关变量）
const actionProgressTitle = ref("");
let actionUnlisten: (() => void) | null = null;

// 根据 lootId 获取友好的显示名称（用于进度面板）
function getFriendlyNameById(lootId: string): string {
  const found = rawLootInventory.value.find(i => i.lootId === lootId);
  return found?.itemDesc ?? lootId;
}

// 加载碎片库存
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

// 过滤后的碎片
const filteredInventory = computed(() => {
  return rawLootInventory.value.filter(item => {
    // 1. 类型过滤
    if (filterType.value !== "ALL" && item.displayCategories !== filterType.value) return false;
    // 2. 拥有状态过滤
    if (filterOwned.value === "OWNED" && item.itemStatus !== "OWNED") return false;
    if (filterOwned.value === "NOT_OWNED" && item.itemStatus === "OWNED") return false;
    // 3. 价值过滤
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

// 全选 / 清空
function handleSelectAllFiltered() {
  selectedLootIds.value = filteredInventory.value.map(item => item.lootId);
}
function handleClearSelection() {
  selectedLootIds.value = [];
}

// 切换单项选中
function toggleSelectItem(lootId: string) {
  const idx = selectedLootIds.value.indexOf(lootId);
  if (idx > -1) {
    selectedLootIds.value.splice(idx, 1);
  } else {
    selectedLootIds.value.push(lootId);
  }
}

// 选中项对应的完整对象
const selectedLootObjects = computed(() => {
  return rawLootInventory.value.filter(item => selectedLootIds.value.includes(item.lootId));
});

// 是否可以升级（选中项中至少存在一个未拥有物品）
const canUpgrade = computed(() => {
  if (selectedLootIds.value.length === 0) return false;
  return selectedLootObjects.value.some(item => item.itemStatus !== "OWNED");
});

// 分解收益估算
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
    onConfirm: proceedWithDisenchant
  };
  showConfirmModal.value = true;
}

async function proceedWithDisenchant() {
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
      loadSummoner(true);
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
    showToast("没有选中任何未拥有的碎片进行升级！", "error");
    return;
  }

  const count = unownedItems.length;
  let totalBlueEssence = 0;
  let totalOrangeEssence = 0;
  unownedItems.forEach(item => {
    if (item.displayCategories === "CHAMPION") {
      totalBlueEssence += item.upgradeEssenceCost * item.count;
    } else {
      totalOrangeEssence += item.upgradeEssenceCost * item.count;
    }
  });

  const detailsList: { label: string; value: string }[] = [
    { label: "待升级碎片种类", value: `${count} 种` }
  ];
  if (totalBlueEssence > 0) {
    detailsList.push({ label: "预计消耗蓝色精粹", value: `🔷 ${totalBlueEssence}` });
  }
  if (totalOrangeEssence > 0) {
    detailsList.push({ label: "预计消耗橙色精粹", value: `🔶 ${totalOrangeEssence}` });
  }
  detailsList.push({ label: "操作说明", value: "将消耗对应精粹并解锁永久版" });

  confirmModalConfig.value = {
    title: t("tools.lootManager.upgradeBtn") || "升级选中项",
    message: `您确定要将选中的 ${count} 个未拥有物品碎片升级为永久吗？`,
    confirmText: "确定升级",
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

  actionProgressTitle.value = "正在升级碎片...";
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
      loadSummoner(true);
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

// ─── 一键三合一重随 ───
function handleBatchReroll() {
  const grouped: Record<string, string[]> = {};
  selectedLootObjects.value.forEach(item => {
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
      loadSummoner(true);
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

// 进度面板标题（区分开启/分解/重随）
const progressPanelTitle = computed(() => {
  return actionProgressTitle.value || t("tools.lootOpener.opening");
});

const openPercentage = computed(() => {
  if (openTotal.value <= 0) return 0;
  return Math.round((openProgress.value / openTotal.value) * 100);
});

const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);
const recentMatches = ref<MatchDisplay[]>([]);
const rankedQueues = ref<any[]>([]);
const loading = ref(false);
const error = ref("");
const copied = ref(false);
const careerGamesNumber = ref(20); // 默认值，启动时从配置读取

// 游戏模式筛选
const selectedQueue = ref<number | null>(null);
const QUEUE_OPTIONS = [
  { id: null, label: "全部" },
  { id: 2400, label: "海克斯大乱斗" },
  { id: 450, label: "极地大乱斗" },
  { id: 430, label: "匹配模式" },
  { id: 420, label: "单双排位" },
  { id: 440, label: "灵活排位" },
];
const showQueueDropdown = ref(false);

const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter(
    (m: MatchDisplay) => m.queueId === selectedQueue.value,
  );
});

function selectQueue(id: number | null) {
  selectedQueue.value = id;
  showQueueDropdown.value = false;
}

// 从 App.vue 注入 Career → Search 跳转状态
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;

const TIER_MAP: Record<string, string> = {
  NONE: "无段位",
  IRON: "坚韧黑铁",
  BRONZE: "英勇黄铜",
  SILVER: "不屈白银",
  GOLD: "荣耀黄金",
  PLATINUM: "华贵铂金",
  EMERALD: "流光翡翠",
  DIAMOND: "璀璨钻石",
  MASTER: "超凡大师",
  GRANDMASTER: "傲世宗师",
  CHALLENGER: "最强王者",
};

async function loadSummoner(forceRefresh = false) {
  // 如果距离上次成功加载小于 20 秒，且已有缓存数据，且不是强刷，则直接使用缓存，不发起 API 请求
  const now = Date.now();
  if (!forceRefresh && cachedSummoner && now - lastFetchedTime < 20000) {
    summoner.value = cachedSummoner;
    rankedQueues.value = cachedRankedQueues;
    matches.value = cachedMatches;
    recentMatches.value = cachedRecentMatches;
    return;
  }

  loading.value = true;
  error.value = "";
  try {
    summoner.value = await fetchCurrentSummoner();
    if (summoner.value?.puuid) {
      await Promise.all([
        loadRankedStats(summoner.value.puuid),
        loadMatches(summoner.value.puuid),
      ]);
      // 异步加载近期统计缓存（不阻塞页面，不 await 避免覆盖翻页数据）
      await loadRecentMatches(summoner.value.puuid);

      // 同步写回模块单例缓存，记录成功拉取时间
      cachedSummoner = summoner.value;
      cachedRankedQueues = rankedQueues.value;
      cachedMatches = matches.value;
      cachedRecentMatches = recentMatches.value;
      lastFetchedTime = Date.now();
    }
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

async function loadRankedStats(puuid: string) {
  try {
    const resp = await lcuRequest<any>(
      "GET",
      `/lol-ranked/v1/ranked-stats/${puuid}`,
    );
    if (resp.success && resp.data && resp.data.queues) {
      rankedQueues.value = resp.data.queues;
      // 同步写回顶级缓存
      cachedRankedQueues = rankedQueues.value;
    }
  } catch (e) {
    console.error("获取排位段位数据失败:", e);
  }
}

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_matches_cache_${puuid}`;

// 公共战绩拉取函数，包含 LCU 数据滞后时的 SGP 加速降级兜底逻辑
async function fetchMatchHistoryWithFallback(
  puuid: string,
  begIndex: number,
  endIndex: number,
  isGameEndSync = false,
): Promise<MatchDisplay[]> {
  let raw = await fetchMatchHistory(puuid, begIndex, endIndex);
  const prevLatestId =
    recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
  const latestId = raw[0]?.gameId ?? null;

  // 只有当 LCU 拉出来为空，或者在游戏刚结束的自动刷新流中且最新 ID 仍为旧的最新 ID（证明 LCU 数据滞后）时，才降级到 SGP 加速拉取
  const shouldFallback =
    raw.length === 0 ||
    (isGameEndSync && prevLatestId && latestId === prevLatestId);

  if (shouldFallback) {
    try {
      const sgpRaw = await fetchMatchHistorySgp(puuid, begIndex, endIndex);
      if (sgpRaw && sgpRaw.length > 0) {
        console.log("[Career] LCU 战绩未更新，已降级 SGP 加速拉取新战绩");
        raw = sgpRaw;
      }
    } catch (e) {
      console.warn("[Career] SGP 战绩降级/加速获取失败:", e);
    }
  }
  return raw;
}

async function loadMatches(puuid: string, isGameEndSync = false) {
  try {
    loading.value = true;
    const targetCount = careerGamesNumber.value;
    matches.value = await fetchMatchHistoryWithFallback(
      puuid,
      0,
      targetCount - 1,
      isGameEndSync,
    );
    // 同步更新顶级缓存
    cachedMatches = matches.value;
  } catch (e) {
    console.error("获取战绩历史失败:", e);
  } finally {
    loading.value = false;
  }
}

async function loadRecentMatches(puuid: string, isGameEndSync = false) {
  try {
    const targetCount = careerGamesNumber.value;
    const fresh = await fetchMatchHistoryWithFallback(
      puuid,
      0,
      targetCount,
      isGameEndSync,
    );
    updateRecentMatchesCache(puuid, fresh);
  } catch (e) {
    console.error("获取近期战绩统计失败:", e);
  }
}

function updateRecentMatchesCache(puuid: string, fresh: MatchDisplay[]) {
  let cached: MatchDisplay[] = [];
  try {
    const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
    if (raw) cached = JSON.parse(raw);
  } catch {
    /* ignore */
  }

  const merged = [...fresh, ...cached]
    .filter(
      (m, idx, arr) => arr.findIndex((x) => x.gameId === m.gameId) === idx,
    )
    .sort((a, b) => b.timeStamp - a.timeStamp)
    .slice(0, careerGamesNumber.value);

  recentMatches.value = merged;
  // 同步更新顶级缓存
  cachedRecentMatches = merged;

  try {
    localStorage.setItem(MATCHES_CACHE_KEY(puuid), JSON.stringify(merged));
  } catch {
    /* ignore */
  }
}

function getKdaClass(kda: string): string {
  const val = parseFloat(kda);
  if (isNaN(val)) return "kda-perfect";
  if (val >= 5) return "kda-great";
  if (val >= 3) return "kda-good";
  return "kda-normal";
}

function formatRank(queue: any) {
  if (!queue || !queue.tier || queue.tier === "NONE") return "--";
  const tierCn = TIER_MAP[queue.tier] || queue.tier;
  const division = queue.rank === "NA" ? "" : " " + queue.rank;
  return `${tierCn}${division}`;
}

function formatHighestRank(queue: any) {
  if (!queue || !queue.highestTier || queue.highestTier === "NONE") return "--";
  return TIER_MAP[queue.highestTier] || queue.highestTier;
}

function formatPrevSeasonRank(queue: any) {
  if (
    !queue ||
    !queue.previousSeasonEndTier ||
    queue.previousSeasonEndTier === "NONE"
  )
    return "--";
  return TIER_MAP[queue.previousSeasonEndTier] || queue.previousSeasonEndTier;
}

// 复制完整 Riot ID
async function copyRiotId() {
  if (!summoner.value) return;
  const fullId = `${summoner.value.gameName || summoner.value.displayName}#${summoner.value.tagLine}`;
  try {
    await navigator.clipboard.writeText(fullId);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    // fallback
    const ta = document.createElement("textarea");
    ta.value = fullId;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1500);
  }
}

// 点击对局卡片 → 跳转到战绩查询页面并自动搜索
function goToMatchDetail(gameId: number) {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId };
}

// 点击历史战绩 → 跳转到战绩查询页面并搜索当前召唤师
function goToHistory() {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  // gameId 设为 -1 表示只搜索不指定对局
  navigateSearchPayload.value = { name: fullName, gameId: -1 };
}

// 召唤师技能图标：直接使用后端解析的 URL（后端已含完整 LCU 路径）
function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
  return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
}

// 读取配置中的对局数量
onMounted(async () => {
  try {
    const cfg = await fetchConfig();
    careerGamesNumber.value = cfg.Functions?.CareerGamesNumber ?? 20;
  } catch (e) {
    console.warn("加载 CareerGamesNumber 配置失败，使用默认值 20:", e);
  }
  document.addEventListener("click", onDocClick);
  if (currentTab.value === 'loot' && store.isConnected) {
    loadOpenableLoots();
    loadLootInventory();
  }
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  cleanupActionListen();
});

function onDocClick() {
  showQueueDropdown.value = false;
}

// 切换到战利品 Tab 时自动加载所有战利品和碎片库存
watch(currentTab, (tab) => {
  if (tab === 'loot' && store.isConnected) {
    loadOpenableLoots();
    loadLootInventory();
  }
});

// 自动加载逻辑
watch(
  () => store.isConnected,
  (connected) => {
    if (connected) {
      loadSummoner();
    } else {
      summoner.value = null;
      matches.value = [];
      recentMatches.value = [];
      rankedQueues.value = [];
      // 断开连接时，同步清空内存缓存，避免恢复连接时加载旧人的数据
      cachedSummoner = null;
      cachedMatches = [];
      cachedRecentMatches = [];
      cachedRankedQueues = [];
      lastFetchedTime = 0;
    }
  },
  { immediate: true },
);

// 对局结束后自动刷新战绩
// 参考: 进入结算/大厅状态后等 2 秒，让 LCU 把对局数据落盘；
// 然后重试 5 次 × 3 秒，因为 lol-match-history 同步新对局通常有几秒到十几秒延迟
watch(
  () => store.gamePhase,
  async (phase: string, oldPhase: string | undefined) => {
    if (!summoner.value?.puuid) return;
    const gamePhases = [
      "InProgress",
      "GameStart",
      "ChampSelect",
      "ReadyCheck",
      "PreEndOfGame",
    ];
    const endPhases = ["EndOfGame", "Lobby", "None"];
    if (
      gamePhases.includes(oldPhase ?? "") &&
      endPhases.includes(phase ?? "")
    ) {
      const puuid = summoner.value.puuid;
      const prevLatestId =
        recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
      console.log(
        `[Career] 对局结束 (${oldPhase} → ${phase})，等待 LCU 同步并重试刷新`,
      );

      // 第一次延迟：让结算界面走完
      await new Promise((r) => setTimeout(r, 2000));

      // 最多重试 5 次，每次间隔 3 秒；只要发现新对局 ID 就提前停止
      for (let attempt = 0; attempt < 5; attempt++) {
        if (attempt > 0) {
          await new Promise((r) => setTimeout(r, 3000));
        }
        try {
          await loadMatches(puuid, true);
          // 异步更新近期统计缓存
          loadRecentMatches(puuid, true);
          const latestId = matches.value[0]?.gameId ?? null;
          if (latestId && latestId !== prevLatestId) {
            console.log(
              `[Career] 第 ${attempt + 1} 次重试时已发现新对局 ${latestId}`,
            );
            return;
          }
          console.log(
            `[Career] 第 ${attempt + 1} 次刷新：尚未发现新对局，继续等待`,
          );
        } catch (e) {
          console.warn(`[Career] 第 ${attempt + 1} 次刷新失败:`, e);
        }
      }
      // 兜底刷新召唤师信息（段位变化等）
      await loadRankedStats(puuid);
    }
  },
);

// 提取排位队列
const soloQueue = computed(() => {
  return (
    rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null
  );
});

const flexQueue = computed(() => {
  return (
    rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null
  );
});

// 计算近期对局统计
const statsSummary = computed(() => {
  if (recentMatches.value.length === 0) return null;
  let wins = 0;
  let losses = 0;
  let kills = 0;
  let deaths = 0;
  let assists = 0;
  const champMap: Record<number, { id: number; icon: string; count: number }> =
    {};

  for (const m of recentMatches.value) {
    if (m.win) wins++;
    else losses++;
    kills += m.kills;
    deaths += m.deaths;
    assists += m.assists;

    if (!champMap[m.championId]) {
      champMap[m.championId] = {
        id: m.championId,
        icon: m.championIconUrl,
        count: 0,
      };
    }
    champMap[m.championId].count++;
  }

  const topChamps = Object.values(champMap)
    .sort((a, b) => b.count - a.count)
    .slice(0, 6);

  const kdaRatio =
    deaths === 0 ? "Perfect" : ((kills + assists) / deaths).toFixed(1);

  return {
    wins,
    losses,
    kills,
    deaths,
    assists,
    kda: kdaRatio,
    topChamps,
  };
});

function formatTime(ts: number): string {
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function translateMapName(name: string): string {
  if (!name) return "";
  if (name.includes("峡谷") || name.includes("Rift")) return t("maps.11");
  if (name.includes("深渊") || name.includes("Abyss")) return t("maps.12");
  if (name.includes("闪击") || name.includes("Blitz")) return t("maps.21");
  if (name.includes("大厅") || name.includes("Lobby")) return t("maps.22");
  return name;
}

function getQueueName(m: MatchDisplay): string {
  const key = `gameModes.${m.queueId}`;
  if (te(key)) {
    const translation = t(key);
    // 翻译防冲突纠错：如果翻译包含“云顶之弈”或“云顶”，但后端实际名称不含“云顶”相关
    // 则说明队列 ID 发生冲突，应该降级显示后端解析出的 name
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !m.name.includes("云顶") &&
      !m.name.includes("TFT")
    ) {
      return m.name;
    }
    return translation;
  }
  return m.name;
}
</script>

<template>
  <div class="career">
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
    </div>

    <div v-else class="career-content">
      <div v-if="error" class="error">{{ error }}</div>

      <!-- 召唤师信息卡片 -->
      <div v-if="summoner" class="summoner-header">
        <div class="profile-center">
          <div class="profile-icon-wrapper">
            <!-- 等级进度环形条（缺口在底部） -->
            <svg class="gauge-ring-svg" viewBox="0 0 100 100">
              <circle class="gauge-track" cx="50" cy="50" r="45" />
              <circle
                class="gauge-progress"
                cx="50"
                cy="50"
                r="45"
                :style="{ '--progress': summoner.percentCompleteForNextLevel }"
              />
            </svg>
            <div class="avatar-container">
              <LcuImage
                :src="summoner.profileIconUrl"
                class="profile-avatar"
                alt="avatar"
              />
            </div>
            <!-- 等级数字（底部缺口处） -->
            <div class="level-badge">{{ summoner.summonerLevel }}</div>
          </div>

          <div class="summoner-info">
            <h1 class="display-name">
              {{ summoner.gameName || summoner.displayName }}
            </h1>
            <div class="copy-wrapper">
              <button
                class="copy-riot-id-btn"
                @click="copyRiotId"
                :title="`复制: ${summoner.gameName || summoner.displayName}#${summoner.tagLine}`"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  class="copy-icon"
                >
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path
                    d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                  />
                </svg>
              </button>
              <span v-if="copied" class="copied-text"
                >✓ {{ $t("career.copied") }}</span
              >
            </div>
            <span class="tagline"># {{ summoner.tagLine }}</span>
          </div>
        </div>

        <div class="header-actions">
          <button
            class="action-btn"
            @click="loadSummoner(true)"
            :disabled="loading"
          >
            {{ $t("career.refresh") }}
          </button>
          <button class="action-btn" @click="goToHistory" :disabled="loading">
            {{ $t("career.historyBtn") }}
          </button>
        </div>
      </div>

      <!-- Tab 导航栏 -->
      <div v-if="summoner" class="career-tabs">
        <div
          :class="['career-tab-item', { active: currentTab === 'matches' }]"
          @click="currentTab = 'matches'"
        >
          生涯战绩
        </div>
        <div
          :class="['career-tab-item', { active: currentTab === 'loot' }]"
          @click="currentTab = 'loot'"
        >
          战利品
        </div>
      </div>

      <div v-show="currentTab === 'matches'" class="matches-tab-container">
        <!-- 排位段位信息表 -->
        <div class="rank-table-wrapper">
        <table class="rank-table">
          <thead>
            <tr>
              <th>{{ $t("career.type") }}</th>
              <th>{{ $t("career.totalGames") }}</th>
              <th>{{ $t("career.winRate") }}</th>
              <th>{{ $t("career.winsLabel") }}</th>
              <th>{{ $t("career.lossesLabel") }}</th>
              <th>{{ $t("career.tier") }}</th>
              <th>{{ $t("career.lp") }}</th>
              <th>{{ $t("career.highest") }}</th>
              <th>{{ $t("career.prevSeason") }}</th>
            </tr>
          </thead>
          <tbody>
            <!-- 单双排 -->
            <tr>
              <td class="type-name">{{ $t("gameModes.420") }}</td>
              <td>{{ soloQueue ? soloQueue.wins + soloQueue.losses : 0 }}</td>
              <td>
                {{
                  soloQueue && soloQueue.wins + soloQueue.losses > 0
                    ? (
                        (soloQueue.wins / (soloQueue.wins + soloQueue.losses)) *
                        100
                      ).toFixed(0) + "%"
                    : "--"
                }}
              </td>
              <td>{{ soloQueue ? soloQueue.wins : 0 }}</td>
              <td>{{ soloQueue ? soloQueue.losses : 0 }}</td>
              <td class="rank-name">
                {{ soloQueue ? formatRank(soloQueue) : "--" }}
              </td>
              <td>{{ soloQueue ? soloQueue.leaguePoints : 0 }}</td>
              <td>{{ soloQueue ? formatHighestRank(soloQueue) : "--" }}</td>
              <td>{{ soloQueue ? formatPrevSeasonRank(soloQueue) : "--" }}</td>
            </tr>
            <!-- 灵活排位 -->
            <tr>
              <td class="type-name">{{ $t("gameModes.440") }}</td>
              <td>{{ flexQueue ? flexQueue.wins + flexQueue.losses : 0 }}</td>
              <td>
                {{
                  flexQueue && flexQueue.wins + flexQueue.losses > 0
                    ? (
                        (flexQueue.wins / (flexQueue.wins + flexQueue.losses)) *
                        100
                      ).toFixed(0) + "%"
                    : "--"
                }}
              </td>
              <td>{{ flexQueue ? flexQueue.wins : 0 }}</td>
              <td>{{ flexQueue ? flexQueue.losses : 0 }}</td>
              <td class="rank-name">
                {{ flexQueue ? formatRank(flexQueue) : "--" }}
              </td>
              <td>{{ flexQueue ? flexQueue.leaguePoints : 0 }}</td>
              <td>{{ flexQueue ? formatHighestRank(flexQueue) : "--" }}</td>
              <td>{{ flexQueue ? formatPrevSeasonRank(flexQueue) : "--" }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 近期数据看板 & 常用英雄 -->
      <div v-if="statsSummary" class="recent-summary-bar">
        <div class="summary-text">
          <span class="summary-title">{{
            $t("career.recentGamesTitle", { count: recentMatches.length })
          }}</span>
          <span class="win-color"
            >{{ $t("career.win") }}: {{ statsSummary.wins }}</span
          >
          <span class="lose-color"
            >{{ $t("career.lose") }}: {{ statsSummary.losses }}</span
          >
          <span class="kda-label">KDA:</span>
          <span class="kda-values">
            {{ statsSummary.kills }} /
            <span class="death-red">{{ statsSummary.deaths }}</span> /
            {{ statsSummary.assists }}
          </span>
          <span class="kda-ratio">({{ statsSummary.kda }})</span>
        </div>

        <div class="recent-champs">
          <div
            v-for="c in statsSummary.topChamps"
            :key="c.id"
            class="recent-champ-icon"
            :title="t('career.gamesCount', { count: c.count })"
          >
            <LcuImage :src="c.icon" alt="champ" />
          </div>
        </div>

        <div class="summary-actions">
          <button class="summary-action-btn">
            {{ $t("career.recentTeammates") }}
          </button>
          <div
            class="dropdown-trigger"
            @click.stop="showQueueDropdown = !showQueueDropdown"
          >
            <span>{{
              selectedQueue === null
                ? $t("career.all")
                : $t("gameModes." + selectedQueue)
            }}</span>
            <svg
              :class="['arrow-icon', { expanded: showQueueDropdown }]"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
            <div
              v-if="showQueueDropdown"
              class="queue-dropdown-menu"
              @click.stop
            >
              <div
                v-for="q in QUEUE_OPTIONS"
                :key="q.id ?? -1"
                :class="[
                  'queue-dropdown-item',
                  { active: selectedQueue === q.id },
                ]"
                @click="selectQueue(q.id)"
              >
                {{ q.id === null ? $t("career.all") : $t("gameModes." + q.id) }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 局部滚动包裹区域：保留头像、排位与近期对局看板，仅滚动对局战绩列表 -->
      <div class="career-scroll-area">
        <!-- 战绩对局历史列表 -->
        <div v-if="filteredMatches.length > 0" class="match-history-list">
          <div
            v-for="m in filteredMatches"
            :key="m.gameId"
            :class="['match-card', m.win ? 'win' : 'lose']"
            @click="goToMatchDetail(m.gameId)"
            style="cursor: pointer"
          >
            <!-- 1. 英雄头像、等级、技能、符文 -->
            <div class="champ-panel">
              <div class="champ-avatar-box">
                <LcuImage
                  :src="m.championIconUrl"
                  class="champ-avatar"
                  alt="champ"
                />
                <div class="level-overlay">{{ m.champLevel }}</div>
              </div>
              <div class="spells-runes">
                <div class="spells-col">
                  <div class="spell-slot">
                    <LcuImage
                      :src="getSpellIcon(m, 1)"
                      class="mini-icon"
                      alt="s1"
                    />
                  </div>
                  <div class="spell-slot">
                    <LcuImage
                      :src="getSpellIcon(m, 2)"
                      class="mini-icon"
                      alt="s2"
                    />
                  </div>
                </div>
                <div v-if="m.queueId !== 2400" class="rune-slot">
                  <LcuImage
                    :src="m.runeIconUrl"
                    class="mini-icon circular"
                    alt="rune"
                  />
                </div>
              </div>
            </div>

            <!-- 2. 胜负状态与游戏模式 -->
            <div class="result-panel">
              <span :class="['result-text', m.win ? 'win-text' : 'lose-text']">
                {{ m.win ? $t("career.victory") : $t("career.defeat") }}
              </span>
              <span class="queue-mode">{{ getQueueName(m) }}</span>
            </div>

            <!-- 3. KDA 数字与文字 -->
            <div class="kda-panel">
              <div class="kda-numbers">
                <span class="bold">{{ m.kills }}</span> /
                <span class="bold death-red">{{ m.deaths }}</span> /
                <span class="bold">{{ m.assists }}</span>
              </div>
              <div class="kda-desc">
                <span class="kda-ratio" :class="getKdaClass(m.kda)"
                  >{{ m.kda }} KDA</span
                >
              </div>
            </div>

            <!-- 4. 补刀补兵数 -->
            <div class="cs-panel">
              <span class="cs-count">{{ m.cs }}</span>
              <svg
                class="cs-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path
                  d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </div>

            <!-- 5. 装备栏 (前 6 件常规装备 + 第 7 件饰品) -->
            <div class="items-panel">
              <div class="items-grid">
                <div v-for="idx in 6" :key="idx" class="item-slot">
                  <LcuImage
                    v-if="m.itemIconUrls[idx - 1]"
                    :src="m.itemIconUrls[idx - 1]"
                    class="item-img"
                    alt="item"
                  />
                </div>
              </div>
              <!-- 饰品独立显示 -->
              <div class="ward-slot">
                <LcuImage
                  v-if="m.itemIconUrls[6]"
                  :src="m.itemIconUrls[6]"
                  class="item-img"
                  alt="ward"
                />
              </div>
            </div>

            <!-- 6. 获得金币 -->
            <div class="gold-panel">
              <span class="gold-count">{{ m.gold.toLocaleString() }}</span>
              <svg class="gold-icon" viewBox="0 0 24 24" fill="currentColor">
                <circle
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  stroke-width="2"
                  fill="none"
                />
                <path
                  d="M12 6v12M15 9H11.5a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H9"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  fill="none"
                />
              </svg>
            </div>

            <!-- 7. 地图模式与时长/日期 -->
            <div class="time-panel">
              <span class="game-map">{{ translateMapName(m.map) }}</span>
              <span class="match-time"
                >{{ m.duration }} · {{ formatTime(m.timeStamp) }}</span
              >
            </div>
          </div>
        </div>
        <div v-else-if="!loading" class="tip-container">
          <p class="tip">{{ $t("career.empty") }}</p>
        </div>
      </div>
      </div>

      <!-- 战利品面板：子 Tab 切换 -->
      <!-- 战利品面板：整页上下平铺 -->
      <div v-show="currentTab === 'loot'" class="loot-tab-container">
        
        <!-- 区块一：可开启战利品（箱子/法球） -->
        <div class="loot-section-card">
          <!-- 头部操作栏 -->
          <div class="loot-section-header">
            <div class="header-left">
              <span class="section-title">📦 {{ $t("tools.lootManager.chestOpen") }}</span>
              <button
                class="action-btn"
                :disabled="!store.isConnected || lootLoading"
                @click="loadOpenableLoots"
              >
                {{ lootLoading ? '正在刷新...' : $t("tools.lootOpener.refreshBtn") }}
              </button>
              <span v-if="openableLoots.length > 0" class="loot-key-summary">
                🔑 ×{{ totalKeyCount }}
              </span>
            </div>
            <button
              v-if="openableLoots.length > 0"
              class="action-btn"
              :disabled="!store.isConnected || isOpening || totalKeyCount <= 0"
              @click="handleSmartOpenAll"
            >
              {{ $t("tools.lootOpener.smartOpenAll") }}
            </button>
          </div>

          <!-- 内容主体 -->
          <div class="loot-section-content">
            <div v-if="lootLoading" class="loot-loading-inline">
              <div class="loading-spinner"></div>
              <span>{{ $t("tools.lootOpener.loading") }}</span>
            </div>
            <div v-else-if="lootError" class="loot-error-inline">{{ lootError }}</div>
            <div v-else-if="sortedOpenableLoots.length > 0" class="loot-grid">
              <div
                v-for="loot in sortedOpenableLoots"
                :key="loot.lootId"
                class="loot-card-item"
                @click="openLootModal(loot)"
              >
                <div class="loot-card-icon-container">
                  <LcuImage :src="loot.tilePath ?? undefined" class="loot-card-icon" />
                </div>
                <div class="loot-card-info">
                  <div class="loot-card-header">
                    <span class="loot-card-name" :title="getLootDisplayName(loot)">{{ getLootDisplayName(loot) }}</span>
                    <span class="loot-card-count">×{{ loot.count }}</span>
                  </div>
                  <div class="loot-card-footer">
                    <span v-if="loot.needKey" class="loot-key-badge">{{ $t("tools.lootOpener.needKey") }}</span>
                    <span v-else class="loot-no-key-badge">{{ $t("tools.lootOpener.noKeyNeeded") }}</span>
                    <span class="loot-open-btn">{{ $t("tools.lootOpener.openBtn") }}</span>
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="loot-empty-inline">{{ $t("tools.lootOpener.clickRefresh") }}</div>
          </div>
        </div>

        <!-- 区块二：碎片库存管理 -->
        <div class="loot-section-card">
          <!-- 头部操作栏 -->
          <div class="loot-section-header">
            <div class="header-left">
              <span class="section-title">💎 {{ $t("tools.lootManager.title") }}</span>
              <button
                class="action-btn"
                :disabled="!store.isConnected || isInventoryLoading"
                @click="loadLootInventory"
              >
                {{ isInventoryLoading ? '正在刷新...' : $t("tools.lootManager.refreshBtn") }}
              </button>
            </div>
            <div class="header-right essence-header-balance">
              <span class="blue-essence-text">🔷 {{ blueEssenceCount }}</span>
              <span class="orange-essence-text">🔶 {{ orangeEssenceCount }}</span>
            </div>
          </div>

          <!-- 紧凑精美的水平筛选栏 -->
          <div class="loot-filter-bar-horizontal">
            <!-- 碎片类型 -->
            <div class="horizontal-filter-item">
              <span class="filter-label-inline">{{ $t("tools.lootManager.filterType") }}</span>
              <n-select
                v-model:value="filterType"
                :options="[
                  { label: $t('tools.lootManager.filterAll'), value: 'ALL' },
                  { label: '英雄', value: 'CHAMPION' },
                  { label: '皮肤', value: 'SKIN' },
                  { label: '表情', value: 'EMOTE' },
                  { label: '守卫', value: 'WARDSKIN' },
                  { label: '图标', value: 'SUMMONERICON' },
                ]"
                size="small"
                style="width: 120px"
              />
            </div>

            <!-- 拥有状态 -->
            <div class="horizontal-filter-item">
              <span class="filter-label-inline">{{ $t("tools.lootManager.filterOwned") }}</span>
              <n-select
                v-model:value="filterOwned"
                :options="[
                  { label: '全部', value: 'ALL' },
                  { label: '已拥有', value: 'OWNED' },
                  { label: '未拥有', value: 'NOT_OWNED' },
                ]"
                size="small"
                style="width: 110px"
              />
            </div>

            <!-- 价值基准 -->
            <div class="horizontal-filter-item">
              <span class="filter-label-inline">{{ $t("tools.lootManager.filterValueType") }}</span>
              <n-select
                v-model:value="filterValueType"
                :options="[
                  { label: $t('tools.lootManager.filterValueTypeDisenchant'), value: 'disenchantValue' },
                  { label: $t('tools.lootManager.filterValueTypeStore'), value: 'value' },
                ]"
                size="small"
                style="width: 120px"
              />
            </div>

            <!-- 价值范围过滤 -->
            <div class="horizontal-filter-item">
              <span class="filter-label-inline">价值</span>
              <n-input-group>
                <n-select
                  v-model:value="filterOperator"
                  :options="[
                    { label: '小于等于 (<=)', value: '<=' },
                    { label: '大于等于 (>=)', value: '>=' },
                    { label: '等于 (=)', value: '=' }
                  ]"
                  size="small"
                  style="width: 115px"
                />
                <n-input-number
                  v-model:value="filterMaxValue"
                  :min="0"
                  placeholder="不限"
                  size="small"
                  clearable
                  style="width: 125px"
                />
              </n-input-group>
            </div>
          </div>

          <!-- 内容主体 -->
          <div class="loot-section-content">
            <!-- 加载态 -->
            <div v-if="isInventoryLoading" class="loot-loading-inline">
              <div class="loading-spinner"></div>
              <span>{{ $t("tools.lootManager.loading") }}</span>
            </div>

            <!-- 错误态 -->
            <div v-else-if="inventoryError" class="loot-error-inline">{{ inventoryError }}</div>

            <!-- 空态 -->
            <div v-else-if="filteredInventory.length === 0" class="loot-empty-inline">
              {{ $t("tools.lootManager.empty") }}
            </div>

            <!-- 碎片卡片网格 -->
            <div v-else class="loot-grid loot-inventory-grid">
              <div
                v-for="item in filteredInventory"
                :key="item.lootId"
                :class="['loot-card-item', { selected: selectedLootIds.includes(item.lootId) }]"
                @click="toggleSelectItem(item.lootId)"
                style="position: relative;"
              >
                <!-- 选中状态角标 -->
                <div v-if="selectedLootIds.includes(item.lootId)" class="selected-checkmark-badge">
                  ✓
                </div>
                <div class="loot-card-icon-container">
                  <LcuImage :src="item.tilePath ?? undefined" class="loot-card-icon" />
                </div>
                <div class="loot-card-info">
                  <div class="loot-card-header">
                    <span class="loot-card-name" :title="item.itemDesc">{{ item.itemDesc }}</span>
                    <span class="loot-card-count">×{{ item.count }}</span>
                  </div>
                  <div class="loot-card-footer">
                    <span :class="item.itemStatus === 'OWNED' ? 'loot-badge-owned' : 'loot-badge-not-owned'">
                      {{ item.itemStatus === 'OWNED' ? $t("tools.lootManager.ownedBadge") : $t("tools.lootManager.notOwnedBadge") }}
                    </span>
                    <span class="essence-badge" :class="item.displayCategories === 'CHAMPION' ? 'blue-essence-text' : 'orange-essence-text'">
                      {{ item.displayCategories === 'CHAMPION' ? '🔷' : '🔶' }} {{ item.disenchantValue }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 底部浮动控制栏 -->
          <div v-if="filteredInventory.length > 0" class="loot-batch-toolbar">
            <div class="toolbar-left">
              <button class="action-btn" @click="handleSelectAllFiltered">
                {{ $t("tools.lootManager.selectAll") }}
              </button>
              <button class="action-btn" @click="handleClearSelection">
                {{ $t("tools.lootManager.clearAll") }}
              </button>
              <span class="selected-count">
                {{ $t("tools.lootManager.selectedInfo", { count: selectedLootIds.length }) }}
              </span>
            </div>
            <div class="toolbar-right">
              <span v-if="selectedLootIds.length > 0" class="essence-preview">
                {{ $t("tools.lootManager.estimateEssence") }}
                <span class="blue-essence-text">🔷 {{ gainBlueEssence }}</span>
                <span class="orange-essence-text">🔶 {{ gainOrangeEssence }}</span>
              </span>
              <button
                class="action-btn"
                :disabled="selectedLootIds.length === 0 || isOpening || !store.isConnected"
                @click="handleBatchDisenchant"
              >
                {{ $t("tools.lootManager.disenchantBtn") }}
              </button>
              <button
                class="action-btn"
                :disabled="selectedLootIds.length === 0 || isOpening || !store.isConnected"
                @click="handleBatchReroll"
              >
                {{ $t("tools.lootManager.rerollBtn") }}
              </button>
              <button
                class="action-btn"
                :disabled="!canUpgrade || isOpening || !store.isConnected"
                @click="handleBatchUpgrade"
              >
                {{ $t("tools.lootManager.upgradeBtn") }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 自定义确认操作弹窗 -->
      <Transition name="fade">
        <div
          v-if="showConfirmModal"
          class="loot-modal-overlay"
          @click.self="showConfirmModal = false"
        >
          <div class="loot-modal-card confirm-modal-card">
            <div class="loot-modal-header confirm-modal-header" :class="confirmModalConfig.type">
              <h3>⚠️ {{ confirmModalConfig.title }}</h3>
              <button class="modal-close-btn" @click="showConfirmModal = false">✕</button>
            </div>
            <div class="loot-modal-body confirm-modal-body">
              <p class="confirm-message">{{ confirmModalConfig.message }}</p>
              
              <!-- 额外详情信息 -->
              <div v-if="confirmModalConfig.details" class="confirm-details-box">
                <div 
                  v-for="(detail, index) in confirmModalConfig.details" 
                  :key="index"
                  class="confirm-detail-row"
                >
                  <span class="detail-label">{{ detail.label }}</span>
                  <span class="detail-value" :class="detail.class">{{ detail.value }}</span>
                </div>
              </div>

              <div class="loot-modal-actions confirm-modal-actions">
                <button class="action-btn" @click="showConfirmModal = false">
                  {{ confirmModalConfig.cancelText }}
                </button>
                <button
                  class="action-btn"
                  @click="executeConfirmedAction"
                >
                  {{ confirmModalConfig.confirmText }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>

      <!-- 战利品开启数量选择弹窗 -->
      <Transition name="fade">
        <div
          v-if="selectedLoot"
          class="loot-modal-overlay"
          @click.self="closeLootModal"
        >
          <div class="loot-modal-card">
            <div class="loot-modal-header">
              <h3>{{ $t("tools.lootOpener.batchOpen") }} - {{ getLootDisplayName(selectedLoot) }}</h3>
              <button class="modal-close-btn" @click="closeLootModal">✕</button>
            </div>
            <div class="loot-modal-body">
              <div class="loot-modal-preview">
                <div class="loot-modal-icon-container">
                  <LcuImage :src="selectedLoot.tilePath ?? undefined" class="loot-modal-icon" />
                </div>
                <div class="loot-modal-details">
                  <span class="loot-modal-name">{{ getLootDisplayName(selectedLoot) }}</span>
                  <span class="loot-modal-owned">{{ $t("tools.lootOpener.owned") }}: ×{{ selectedLoot.count }}</span>
                </div>
              </div>
              <div v-if="selectedLoot.needKey && selectedLoot.keyLootId" class="loot-info-row">
                <span class="loot-info-label">{{ $t("tools.lootOpener.keyRequired") }}</span>
                <span class="loot-info-value loot-key-count">
                  {{ keyDisplayName }} ×{{ currentKeyCount }}
                </span>
              </div>
              <div class="loot-quantity-row">
                <span class="loot-info-label">{{ $t("tools.lootOpener.quantity") }}</span>
                <n-input-number
                  v-model:value="openQuantity"
                  :min="1"
                  :max="maxOpenQuantity"
                  style="width: 120px"
                  size="small"
                />
              </div>
              <div v-if="maxOpenQuantity <= 0 && selectedLoot.needKey" class="loot-insufficient">
                {{ $t("tools.lootOpener.insufficientKeys") }}
              </div>
              <div class="loot-modal-actions">
                <button class="action-btn" @click="closeLootModal">
                  {{ $t("tools.cancel") }}
                </button>
                <button
                  class="action-btn"
                  :disabled="maxOpenQuantity <= 0"
                  @click="handleBatchOpen"
                >
                  {{ $t("tools.lootOpener.startOpen", { count: openQuantity }) }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>

      <!-- 批量开启进度面板 -->
      <Transition name="slide-up">
        <div v-if="showOpenPanel" class="loot-progress-overlay">
          <div class="loot-progress-card">
            <div class="loot-progress-header">
              <h3>{{ progressPanelTitle }}</h3>
              <button
                class="modal-close-btn"
                :disabled="isOpening"
                @click="closeOpenPanel"
              >✕</button>
            </div>
            <div class="loot-progress-body">
              <n-progress
                type="line"
                :percentage="openPercentage"
                :indicator-placement="'inside'"
                :border-radius="4"
                :height="20"
                status="success"
              />
              <div class="loot-progress-info">
                {{ openProgress }} / {{ openTotal }}
              </div>
              <div class="loot-results-list">
                <div
                  v-for="(result, idx) in openResults"
                  :key="idx"
                  :class="['loot-result-item', result.success ? 'success' : 'error']"
                >
                  <span class="loot-result-icon">{{ result.success ? '🎉' : '❌' }}</span>
                  <span class="loot-result-text">
                    [{{ result.current }}/{{ result.total }}]
                    {{ result.success ? result.rewardName : result.errorMsg }}
                  </span>
                </div>
              </div>
              <div v-if="!isOpening" class="loot-progress-actions" style="margin-top: 14px;">
                <n-button type="primary" size="medium" block @click="closeOpenPanel">
                  确定
                </n-button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.career {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

.error {
  color: var(--loss-color);
  background: var(--loss-bg);
  border: 1px solid var(--loss-border);
  padding: 10px 16px;
  border-radius: 6px;
  margin-bottom: 1rem;
}

.career-content {
  max-width: 1000px;
  width: 100%;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.matches-tab-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

/* 战绩及看板局部滚动区域 */
.career-scroll-area {
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
  margin-top: 1rem;
}

/* 召唤师头部卡片 */
.summoner-header {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 1.5rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  margin-bottom: 1.5rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.profile-center {
  display: flex;
  align-items: center;
  gap: 1.2rem;
  justify-content: center;
  grid-column: 2;
}

.summoner-header:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
  background-color: var(--card-bg-hover);
}

.profile-icon-wrapper {
  position: relative;
  width: 104px;
  height: 104px;
  flex-shrink: 0;
}

.gauge-ring-svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

/* 环形轨道（灰色）与进度（主题色）共享参数 */
/* r=45, 周长=2π×45≈282.74, 缺口60°以容纳3位等级, 可见弧≈235.62 */
/* 旋转120°使缺口居中于底部(90°) */
.gauge-track,
.gauge-progress {
  fill: none;
  stroke-width: 5;
  stroke-linecap: round;
  stroke-dasharray: 235.62 282.74;
  transform: rotate(120deg);
  transform-origin: center;
}

.gauge-track {
  stroke: #d9d9d9;
}

.gauge-progress {
  stroke: var(--primary-color);
  stroke-dashoffset: calc(235.62px * (1 - var(--progress) / 100));
  transition: stroke-dashoffset 0.8s ease;
}

.avatar-container {
  position: absolute;
  inset: 13px;
  border-radius: 50%;
  overflow: hidden;
}

.profile-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.level-badge {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.72rem;
  font-weight: 700;
  z-index: 2;
  white-space: nowrap;
  line-height: 1;
}

[data-theme="dark"] .level-badge {
  color: var(--text-color);
}

.summoner-info {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  gap: 4px 8px;
  flex: 1;
}

.display-name {
  grid-column: 1;
  grid-row: 1;
  font-size: 1.6rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  text-align: center;
}

.tagline {
  grid-column: 1;
  grid-row: 2;
  font-size: 0.85rem;
  color: var(--text-muted);
  text-align: center;
}

.copy-wrapper {
  grid-column: 2;
  grid-row: 1 / 3;
  align-self: center;
  justify-self: end;
  position: relative;
  display: flex;
  align-items: center;
}

.copy-riot-id-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.02);
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s;
  padding: 0;
}

.copy-riot-id-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.copy-icon {
  width: 14px;
  height: 14px;
}

.copied-text {
  position: absolute;
  left: calc(100% + 6px);
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.75rem;
  color: var(--win-color);
  font-weight: 600;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  grid-column: 3;
  justify-self: end;
}

.action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}

.action-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}


/* 排位数据表 */
.rank-table-wrapper {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: 1.5rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.rank-table-wrapper:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.rank-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.rank-table th,
.rank-table td {
  padding: 12px 16px;
  font-size: 0.82rem;
  border-bottom: 1px solid var(--border-color);
}

.rank-table th {
  background-color: rgba(0, 0, 0, 0.01);
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1.5px solid var(--border-color);
}

.rank-table tr:last-child td {
  border-bottom: none;
}

.type-name {
  font-weight: 600;
  color: var(--text-color);
}

.rank-name {
  font-weight: bold;
  color: var(--primary-color);
}

/* 近期数据概览栏 */
.recent-summary-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background-color: var(--settings-collapse-bg, var(--card-bg)) !important;
  backdrop-filter: blur(15px) !important;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
  position: sticky;
  top: 0;
  z-index: 100 !important;
}

.recent-summary-bar:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.summary-text {
  font-size: 0.82rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-title {
  font-weight: bold;
  color: var(--text-color);
  margin-right: 4px;
}

.win-color {
  color: var(--win-color);
  font-weight: 600;
}

.lose-color {
  color: var(--loss-color);
  font-weight: 600;
}

.kda-label {
  color: var(--text-dimmed);
  margin-left: 8px;
}

.kda-values {
  color: var(--text-color);
  font-weight: 600;
}

.death-red {
  color: var(--death-color, var(--loss-color));
}

.kda-ratio {
  color: var(--text-muted);
  font-size: 0.8rem;
}

.recent-champs {
  display: flex;
  gap: 4px;
}

.recent-champ-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.summary-action-btn:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.78rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
}

.dropdown-trigger:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  transition: transform 0.2s;
}

.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}

/* 模式筛选下拉菜单 */
.queue-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}

.queue-dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.queue-dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.arrow-icon {
  width: 12px;
  height: 12px;
}

/* 战绩对局历史卡片 */
.match-history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.match-card {
  display: flex;
  align-items: center;
  padding: 14px 20px;
  border-radius: 10px;
  border: 1px solid var(--border-color);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  cursor: pointer;
}

.match-card:hover {
  box-shadow: var(--shadow-md);
}

.match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.match-card.win:hover {
  background-color: rgba(16, 185, 129, 0.12);
}

.match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.11);
}

/* 1. 英雄面板 */
.champ-panel {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 120px;
}

.champ-avatar-box {
  position: relative;
  width: 52px;
  height: 52px;
}

.champ-avatar {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  overflow: hidden;
  border: 1.5px solid var(--border-color);
}

.level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 18px;
  height: 18px;
  line-height: 16px;
  background-color: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.7rem;
  font-weight: 700;
  text-align: center;
  border: 1px solid var(--border-color);
}

[data-theme="dark"] .level-overlay {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255, 255, 255, 0.15);
}

.spells-runes {
  display: flex;
  gap: 3px;
  align-items: center;
}

.spells-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spell-slot,
.rune-slot {
  width: 22px;
  height: 22px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.mini-icon {
  width: 100%;
  height: 100%;
  display: block;
}

.mini-icon.circular {
  border-radius: 50%;
}

/* 2. 胜负面板 */
.result-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 100px;
}

.result-text {
  font-size: 1rem;
  font-weight: 800;
}

.win-text {
  color: var(--win-color);
}

.lose-text {
  color: var(--loss-color);
}

.queue-mode {
  font-size: 0.8rem;
  color: var(--text-muted);
  margin-top: 2px;
}

/* 3. KDA面板 */
.kda-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 130px;
}

.kda-numbers {
  font-size: 1rem;
  color: var(--text-muted);
}

.bold {
  font-weight: 700;
  color: var(--text-color);
}

.kda-desc {
  margin-top: 2px;
}

.kda-ratio {
  font-size: 0.82rem;
  font-weight: 700;
}

.kda-perfect {
  color: #d97706;
}
.kda-great {
  color: #db2777;
}
.kda-good {
  color: #2563eb;
}
.kda-normal {
  color: var(--text-dimmed);
}

/* 4. 补刀面板 */
.cs-panel {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 75px;
}

.cs-count {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-muted);
}

.cs-icon {
  width: 16px;
  height: 16px;
  color: var(--text-dimmed);
}

/* 5. 装备面板 */
.items-panel {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
}

.items-grid {
  display: flex;
  gap: 2px;
}

.item-slot,
.ward-slot {
  width: 28px;
  height: 28px;
  background-color: rgba(0, 0, 0, 0.04);
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.ward-slot {
  border-color: rgba(245, 158, 11, 0.3);
  background-color: rgba(245, 158, 11, 0.05);
  margin-left: 2px;
}

/* 6. 金币面板 */
.gold-panel {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 85px;
  justify-content: flex-end;
}

.gold-count {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-muted);
}

.gold-icon {
  width: 14px;
  height: 14px;
  color: #fbbf24;
}

/* 7. 时间面板 */
.time-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  min-width: 170px;
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.map-name {
  font-weight: 600;
  color: var(--text-muted);
}

.match-time {
  margin-top: 4px;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
/* ─── 导航 Tab 样式 ─── */
.career-tabs {
  display: flex;
  gap: 8px;
  margin: 12px 0 16px 0;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 8px;
}

.career-tab-item {
  font-size: 0.88rem;
  font-weight: 700;
  color: var(--text-muted);
  padding: 6px 16px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  background: transparent;
}

.career-tab-item:hover {
  color: var(--primary-color);
  background: var(--card-bg-hover);
}

.career-tab-item.active {
  color: var(--primary-color);
  background: var(--primary-color-alpha-10);
  box-shadow: inset 0 0 0 1px var(--primary-color-alpha-20);
}

/* ─── 战利品批量开启样式 ─── */
.loot-tab-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
}

.loot-action-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: var(--shadow-sm);
  backdrop-filter: blur(8px);
}

.action-bar-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.loot-loading,
.loot-error,
.loot-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 3rem;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
}

.loot-error {
  color: var(--loss-color);
}

.loot-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 16px;
  padding-bottom: 24px;
}

.loot-card-item {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 12px 14px;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: flex;
  align-items: center;
  gap: 14px;
  box-shadow: var(--shadow-sm);
}

.loot-card-item:hover {
  border-color: var(--primary-color);
  box-shadow: var(--shadow-md), 0 0 0 1px var(--primary-color-alpha-30);
  transform: translateY(-2px);
  background: var(--card-bg-hover);
}

.loot-card-icon-container {
  width: 52px;
  height: 52px;
  border-radius: 10px;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.05);
  border: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.loot-card-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.loot-card-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.loot-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-card-name {
  font-size: 0.85rem;
  font-weight: 800;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.loot-card-count {
  font-size: 0.78rem;
  font-weight: 800;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  padding: 2px 8px;
  border-radius: 6px;
  margin-left: 8px;
  white-space: nowrap;
}

.loot-card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-key-badge {
  font-size: 0.7rem;
  color: var(--warning-color, #e6a23c);
  background: var(--warning-color-alpha-10, rgba(230, 162, 60, 0.1));
  border: 1px solid var(--warning-color-alpha-20, rgba(230, 162, 60, 0.2));
  padding: 2px 8px;
  border-radius: 4px;
  font-weight: 600;
}

.loot-no-key-badge {
  font-size: 0.7rem;
  color: var(--text-dimmed);
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
}

.loot-open-btn {
  font-size: 0.82rem;
  font-weight: 600;
  padding: 6px 16px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  color: var(--text-color);
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}

.loot-open-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.loot-key-summary {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--warning-color, #e6a23c);
  background: var(--warning-color-alpha-10, rgba(230, 162, 60, 0.1));
  padding: 4px 12px;
  border-radius: 8px;
  border: 1px solid var(--warning-color-alpha-20, rgba(230, 162, 60, 0.2));
}

/* 数量选择弹窗 */
.loot-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.loot-modal-card {
  width: 380px;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-modal-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-modal-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.loot-modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.loot-modal-preview {
  display: flex;
  align-items: center;
  gap: 16px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  padding: 12px;
  border-radius: 10px;
}

.loot-modal-icon-container {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.loot-modal-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.loot-modal-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-modal-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-modal-owned {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.loot-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-info-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.loot-info-value {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-color);
}

.loot-key-count {
  color: var(--warning-color, #e6a23c);
}

.loot-quantity-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 0;
  border-top: 1px dashed var(--border-color);
  border-bottom: 1px dashed var(--border-color);
}

.loot-insufficient {
  font-size: 0.78rem;
  color: var(--loss-color);
  font-weight: 600;
  text-align: center;
}

.loot-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding-top: 6px;
}

/* 批量开启进度面板 */
.loot-progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.loot-progress-card {
  width: 420px;
  max-height: 80vh;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-progress-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-progress-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
}

.loot-progress-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
}

.loot-progress-info {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  font-weight: 600;
  text-align: center;
}

.loot-results-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-result-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 0.78rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  animation: lootResultIn 0.3s ease-out;
}

.loot-result-item.success {
  border-color: var(--win-border);
  background: var(--win-bg);
}

.loot-result-item.error {
  border-color: var(--loss-border);
  background: var(--loss-bg);
}

.loot-result-icon {
  font-size: 0.9rem;
  flex-shrink: 0;
}

.loot-result-text {
  color: var(--text-color);
  word-break: break-all;
}

@keyframes lootResultIn {
  from {
    opacity: 0;
    transform: translateX(-8px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

/* ═════════ 战利品区块卡片 & 碎片库存管理 ═════════ */

.loot-section-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  box-shadow: var(--shadow-sm);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}

.loot-section-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md), 0 4px 20px rgba(0, 0, 0, 0.02);
}

.loot-section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 14px;
}

.essence-header-balance {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 0.85rem;
  font-weight: 700;
  background: var(--hover-bg);
  padding: 4px 12px;
  border-radius: 20px;
  border: 1px solid var(--border-color);
}

.loot-section-header .header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.section-title {
  font-size: 1rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-section-content {
  min-height: 80px;
  display: flex;
  flex-direction: column;
}

/* 水平筛选栏 */
.loot-filter-bar-horizontal {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  align-items: center;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 16px;
}

.horizontal-filter-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label-inline {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  white-space: nowrap;
}

/* 碎片卡片选择态 */
.loot-card-item.selected {
  border-color: var(--primary-color) !important;
  box-shadow: 0 8px 24px var(--primary-color-alpha-30), 0 0 0 2px var(--primary-color) !important;
  background: var(--primary-color-alpha-15) !important;
  transform: translateY(-4px) scale(1.02) !important;
}

/* 选中标记角标 */
.selected-checkmark-badge {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  background: var(--primary-color);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 900;
  box-shadow: 0 2px 8px var(--primary-color-alpha-40);
  border: 1.5px solid #ffffff;
  z-index: 10;
  animation: popIn 0.25s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes popIn {
  from {
    transform: scale(0);
  }
  to {
    transform: scale(1);
  }
}

/* 精品率/拥有状态标签 */
.loot-badge-owned {
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.25);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
}

.loot-badge-not-owned {
  background: rgba(107, 114, 128, 0.1);
  color: var(--text-muted);
  border: 1px solid rgba(107, 114, 128, 0.2);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
}

/* 精粹数值 */
.essence-badge {
  font-size: 0.72rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.blue-essence-text { color: #2563eb; }
.orange-essence-text { color: #d97706; }

/* 内置加载/错误/空态 */
.loot-loading-inline,
.loot-error-inline,
.loot-empty-inline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  width: 100%;
  box-sizing: border-box;
}

.loot-error-inline {
  color: var(--loss-color);
}

/* 底部浮动控制栏 */
.loot-batch-toolbar {
  position: sticky;
  bottom: 0;
  background: var(--settings-card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 12px 18px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  box-shadow: var(--shadow-lg);
  z-index: 10;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  margin-top: 16px;
}

.loot-batch-toolbar .toolbar-left,
.loot-batch-toolbar .toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.selected-count {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  margin-left: 6px;
}

.essence-preview {
  font-size: 0.82rem;
  font-weight: 700;
  display: flex;
  gap: 8px;
  align-items: center;
  margin-right: 6px;
}

/* 碎片网格负边距修正 */
.loot-inventory-grid {
  padding-bottom: 4px;
}

/* 自定义确认弹窗特殊样式 */
.confirm-modal-card {
  width: 350px !important;
}

.confirm-message {
  font-size: 0.85rem;
  color: var(--text-color);
  line-height: 1.5;
  margin: 0 0 12px 0;
}

.confirm-details-box {
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.confirm-detail-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.78rem;
}

.confirm-detail-row .detail-label {
  color: var(--text-muted);
}

.confirm-detail-row .detail-value {
  font-weight: 700;
  color: var(--text-color);
}
</style>
