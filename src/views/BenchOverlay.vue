<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { lcuRequest, fetchConfig } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const store = useLcuStore();
const pickableIds = ref<number[]>([]);

// 大乱斗选人初始时间戳与英雄加入板凳席的时间映射
const sessionStartTimestamp = ref<number>(Date.now());

// 本局我曾拥有过的英雄ID（由Rust通过bench-my-champion事件直接推送，绕过webview隔离）
const myHistoricalChampions = ref<number[]>([]);
const previousMyChampionId = ref<number>(0);
let unlistenMyChampion: (() => void) | undefined;

// 响应式当前时间，用于毫秒级平滑解冻保护期
const currentNow = ref<number>(Date.now());
let timeTicker: number | undefined;

onMounted(async () => {
  timeTicker = window.setInterval(() => {
    currentNow.value = Date.now();
  }, 100);

  // 监听Rust直接推送的"我的英雄"事件（跨Webview通信，处理未来的英雄变化）
  unlistenMyChampion = await listen<number>("bench-my-champion", (event) => {
    const cid = Number(event.payload);
    if (cid > 0 && !myHistoricalChampions.value.includes(cid)) {
      myHistoricalChampions.value = [...myHistoricalChampions.value, cid];
      console.log(`[BenchOverlay] Rust推送英雄到本地记录: ${cid}`, myHistoricalChampions.value);
    }
  });

  // 主动从Rust AppState拉取本局历史英雄（核心兜底：即使事件在mounted前已发出也不会丢失）
  try {
    const cached = await invoke<number[]>("get_bench_my_champions");
    if (cached && cached.length > 0) {
      const next = [...myHistoricalChampions.value];
      let updated = false;
      cached.forEach(cid => {
        if (cid > 0 && !next.includes(cid)) {
          next.push(cid);
          updated = true;
        }
      });
      if (updated) {
        myHistoricalChampions.value = next;
        console.log(`[BenchOverlay] 从Rust缓存恢复历史英雄:`, myHistoricalChampions.value);
      }
    }
  } catch (e) {
    console.warn("[BenchOverlay] get_bench_my_champions 调用失败:", e);
  }
});

onUnmounted(() => {
  if (timeTicker) window.clearInterval(timeTicker);
  unlistenMyChampion?.();
});

// 获取当前大乱斗板凳席的英雄
const benchChampions = computed(() => {
  const rawList = store.champSelectSession?.benchChampions || [];
  return rawList.filter((c: any) => c && c.championId);
});

// 获取当前玩家在该选人会话中可用的英雄 ID 列表
async function fetchPickableIds(retryCount = 3) {
  console.log(
    `[BenchOverlay] 正在获取当前可用英雄列表... (剩余重试次数: ${retryCount})`,
  );
  const resp = await lcuRequest<number[]>(
    "GET",
    "/lol-champ-select/v1/pickable-champion-ids",
  );
  if (resp.success && Array.isArray(resp.data)) {
    pickableIds.value = resp.data;
    console.log(
      `[BenchOverlay] 可用英雄列表获取成功，共 ${resp.data.length} 个英雄`,
    );
  } else {
    console.error("[BenchOverlay] 获取可用英雄列表失败:", resp.error);
    const errStr = String(resp.error || "");
    if (errStr.includes("404") || errStr.includes("No active delegate")) {
      console.log("[BenchOverlay] 检测到选人会话未激活，静默终止重试");
      return;
    }

    if (retryCount > 0) {
      console.log("[BenchOverlay] 2秒后重试拉取可用英雄列表...");
      setTimeout(() => {
        fetchPickableIds(retryCount - 1);
      }, 2000);
    }
  }
}

onMounted(async () => {
  fetchPickableIds();

  // 同步主程序的主题色到悬浮窗子窗口
  try {
    const cfg = await fetchConfig();
    const mode = cfg?.Personalization?.ThemeMode;
    let theme = "light";
    if (mode === "Dark") {
      theme = "dark";
    } else if (mode === "Light") {
      theme = "light";
    } else {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      theme = isDark ? "dark" : "light";
    }
    document.documentElement.setAttribute("data-theme", theme);
    console.log(`[BenchOverlay] 悬浮窗成功应用主题: ${theme}`);
  } catch (e) {
    console.warn("[BenchOverlay] 同步主窗口主题色失败:", e);
  }
});

// 判断某个英雄当前玩家是否可用（拥有或周免）
const isPickable = (championId: number) => {
  if (pickableIds.value.length === 0) return true;
  return pickableIds.value.includes(championId);
};

// 经由官方 API 验证成功属于本人的动态点亮英雄 ID 集合
const unlockedMyBenchChampions = ref<Set<number>>(new Set());

// 前 15 秒准备阶段保护期时长（毫秒）
const FIRST_STAGE_PROTECT_MS = 15000;

// 计算板凳英雄的状态，供 UI 置灰与 tooltip 提示使用
const getProtectionState = (champ: any): { clickable: boolean; reason: string } => {
  const cid = Number(champ.championId);

  if (!isPickable(cid)) {
    return { clickable: false, reason: "未拥有/不可用" };
  }

  // 经由官方 API 置换成功验证属于本人的专属英雄，在前 15 秒内保持动态点亮高亮
  if (unlockedMyBenchChampions.value.has(cid)) {
    return { clickable: true, reason: "点击秒抢" };
  }

  const timer = store.champSelectSession?.timer as any;
  const phase = timer?.phase || "";

  // 使用官方 session.timer 精准计算已过去的时间
  let isFirst15Seconds = true;
  if (timer && typeof timer.totalTimeInPhase === "number" && timer.totalTimeInPhase > 0) {
    const totalTime = timer.totalTimeInPhase;
    const timeLeft = timer.adjustedTimeLeftInPhase || 0;
    const elapsedInPhase = totalTime - timeLeft;
    if (elapsedInPhase >= FIRST_STAGE_PROTECT_MS) {
      isFirst15Seconds = false;
    }
  } else {
    const elapsedFromStart = currentNow.value - sessionStartTimestamp.value;
    if (elapsedFromStart >= FIRST_STAGE_PROTECT_MS) {
      isFirst15Seconds = false;
    }
  }

  // 1. 倒计时 timer.phase === "FINALIZATION" 或已满 15 秒：
  //    全面开放阶段，板凳席英雄全部高亮显示
  if (phase === "FINALIZATION" || !isFirst15Seconds) {
    return { clickable: true, reason: "点击秒抢" };
  }

  // 2. 前 15 秒准备阶段：初始置灰提示，点击校验成功后动态点亮
  return { clickable: false, reason: "前 15 秒准备阶段 (队友保护期中)" };
};

// 判断板凳英雄是否可点击（满 15 秒后或动态点亮后高亮）
const isBenchChampionClickable = (champ: any) => getProtectionState(champ).clickable;

// 获取提示文案
const getDisabledReason = (champ: any) => getProtectionState(champ).reason;

// 底部迷你提示（小窗口内替代 Naive UI 居中大 toast，避免遮挡英雄）
const hintText = ref("");
let hintTimer: number | undefined;
function showHint(msg: string) {
  hintText.value = msg;
  if (hintTimer) window.clearTimeout(hintTimer);
  hintTimer = window.setTimeout(() => {
    hintText.value = "";
  }, 1500);
}

// 点击选择/抢下板凳席英雄
async function swapChampion(champ: any) {
  const cid = Number(champ.championId);
  
  if (!isPickable(cid)) {
    console.warn(`[BenchOverlay] 未拥有/不可用该英雄，跳过: ${cid}`);
    showHint("未拥有/不可用");
    return;
  }

  console.log(`[BenchOverlay] 发送交换请求 ID: ${cid}`);

  const resp = await lcuRequest(
    "POST",
    `/lol-champ-select/v1/session/bench/swap/${cid}`,
    {},
  );

  if (resp.success) {
    console.log(`[BenchOverlay] 抢/换英雄成功: ${cid}`);
    // 验证成功，将该英雄及换下的旧英雄强行加入动态点亮集合
    unlockedMyBenchChampions.value.add(cid);
    if (previousMyChampionId.value > 0) {
      unlockedMyBenchChampions.value.add(previousMyChampionId.value);
    }
  } else {
    console.warn(`[BenchOverlay] 抢/换英雄失败: ${cid}, 错误:`, resp.error);
    showHint("前 15 秒保护期");
  }
}

// 关闭悬浮窗
function closeOverlay() {
  getCurrentWindow().close();
}

// 开始拖动窗口
function startDrag() {
  getCurrentWindow()
    .startDragging()
    .catch((err) => {
      console.warn("[BenchOverlay] 启动拖动失败:", err);
    });
}

// 监听对局阶段变化，重置开始时间戳与板凳席时间映射
watch(
  () => store.gamePhase,
  (phase, prevPhase) => {
    // 只有当确切的上一个阶段存在且不是 ChampSelect 时（即跨阶段跃迁），才重置
    if (phase === "ChampSelect" && prevPhase && prevPhase !== "ChampSelect") {
      sessionStartTimestamp.value = Date.now();
      myHistoricalChampions.value = [];
      previousMyChampionId.value = 0;
      unlockedMyBenchChampions.value.clear();
      console.log("[BenchOverlay] 跨阶段进入选人阶段，初始化开始时间戳与动态点亮库");
    }
  },
  { immediate: true }
);

watch(
  () => store.champSelectSession,
  async (session) => {
    console.log("[BenchOverlay] LCU Session 改变:", session);
    if (!session) {
      return;
    }
    console.log("[BenchOverlay Session Dump] Raw Session Object:", JSON.parse(JSON.stringify(session)));
    
    // 从session持续更新我的历史英雄（含换下退至板凳席的旧英雄追踪）
    const myPlayer = session.myTeam?.find(
      (p: any) => Number(p.cellId) === Number(session.localPlayerCellId)
    );
    const myCid = Number(myPlayer?.championId || myPlayer?.championPickIntent || 0);

    // 如果之前有手持英雄，且与当前 myCid 不同，说明旧英雄已经退回到板凳席或被替换
    if (previousMyChampionId.value > 0 && previousMyChampionId.value !== myCid) {
      if (!myHistoricalChampions.value.includes(previousMyChampionId.value)) {
        myHistoricalChampions.value = [...myHistoricalChampions.value, previousMyChampionId.value];
        console.log(`[BenchOverlay] 录入被替换放回板凳席的旧英雄: ${previousMyChampionId.value}`, myHistoricalChampions.value);
      }
    }

    if (myCid > 0) {
      previousMyChampionId.value = myCid;
      if (!myHistoricalChampions.value.includes(myCid)) {
        myHistoricalChampions.value = [...myHistoricalChampions.value, myCid];
        console.log(`[BenchOverlay] session更新，录入我的当前英雄: ${myCid}`, myHistoricalChampions.value);
      }
    }

    console.log(
      "[BenchOverlay] 板凳席状态: benchEnabled =",
      session.benchEnabled,
      "benchChampions =",
      session.benchChampions,
      "phase =",
      session.timer?.phase,
    );
    if (pickableIds.value.length === 0) {
      console.log("[BenchOverlay] 选人会话更新且可用列表为空，补充拉取...");
      fetchPickableIds();
    }
  },
  { immediate: true },
);

</script>

<template>
  <div class="bench-overlay-wrapper">
    <!-- 拖动手柄区 -->
    <div
      class="drag-handle"
      @mousedown="startDrag"
      data-tauri-drag-region
      title="拖动调整位置"
    >
      <svg
        class="drag-icon"
        style="pointer-events: none"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <circle cx="9" cy="5" r="1" />
        <circle cx="9" cy="12" r="1" />
        <circle cx="9" cy="19" r="1" />
        <circle cx="15" cy="5" r="1" />
        <circle cx="15" cy="12" r="1" />
        <circle cx="15" cy="19" r="1" />
      </svg>
    </div>

    <!-- 英雄头像列表 -->
    <div class="bench-list">
      <div v-if="benchChampions.length === 0" class="empty-hint">
        板凳席空空如也
      </div>

      <div
        v-for="champ in benchChampions"
        :key="champ.championId"
        :class="['champ-item', { disabled: !isBenchChampionClickable(champ) }]"
        @click="swapChampion(champ)"
        :title="getDisabledReason(champ)"
      >
        <LcuImage
          :src="`/lol-game-data/assets/v1/champion-icons/${champ.championId}.png`"
          class="champ-avatar"
        />
      </div>
    </div>

    <!-- 关闭按钮 -->
    <div class="close-btn" @click="closeOverlay" title="隐藏浮窗">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </div>

    <!-- 底部迷你提示条（不遮挡英雄） -->
    <transition name="hint-fade">
      <div v-if="hintText" class="bench-hint">{{ hintText }}</div>
    </transition>
  </div>
</template>

<style scoped>
.bench-overlay-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  width: 100vw;
  height: 100vh;
  padding: 5px 10px;
  background: var(--card-bg);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
  color: var(--text-color);
  overflow: hidden;
  transition:
    background 0.3s,
    border-color 0.3s;
}

/* 拖动手柄 */
.drag-handle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 100%;
  cursor: move;
  opacity: 0.5;
  color: var(--text-dimmed);
  transition:
    opacity 0.2s,
    color 0.2s;
}
.drag-handle:hover {
  opacity: 1;
  color: var(--text-color);
}
.drag-icon {
  width: 16px;
  height: 16px;
}

/* 英雄列表 */
.bench-list {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  overflow-x: auto;
}

.empty-hint {
  font-size: 12px;
  color: var(--text-dimmed);
}

.champ-item {
  position: relative;
  width: 44px;
  height: 44px;
  border-radius: 8px;
  overflow: hidden;
  border: 2px solid var(--border-color);
  cursor: pointer;
  transition:
    transform 0.15s,
    border-color 0.15s;
}

.champ-item:hover {
  transform: scale(1.1);
  border-color: var(--primary-color);
}

.champ-item.disabled {
  filter: grayscale(1) opacity(0.4);
  cursor: not-allowed;
  border-color: var(--border-color);
}

.champ-item.disabled:hover {
  transform: none;
  border-color: var(--border-color);
}

.champ-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* 关闭按钮 */
.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  cursor: pointer;
  opacity: 0.5;
  color: var(--text-dimmed);
  transition:
    opacity 0.2s,
    color 0.2s;
}
.close-btn:hover {
  opacity: 1;
  color: #f43f5e;
}
.close-btn svg {
  width: 16px;
  height: 16px;
}

/* 底部迷你提示条 */
.bench-hint {
  position: absolute;
  bottom: 4px;
  left: 50%;
  transform: translateX(-50%);
  max-width: 90%;
  padding: 2px 10px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 11px;
  line-height: 1.6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  pointer-events: none;
  z-index: 10;
}

.hint-fade-enter-active,
.hint-fade-leave-active {
  transition: opacity 0.25s;
}
.hint-fade-enter-from,
.hint-fade-leave-to {
  opacity: 0;
}
</style>
