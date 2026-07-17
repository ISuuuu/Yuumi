<script setup lang="ts">
import { ref, onMounted, computed, watch } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { lcuRequest, fetchConfig } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

const store = useLcuStore();
const pickableIds = ref<number[]>([]);

// 大乱斗选人初始时间戳与英雄加入板凳席的时间映射
const sessionStartTimestamp = ref<number>(Date.now());
const benchChampionsAddedTime = ref<Record<number, number>>({});

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

// 判断板凳英雄是否可点击
const isBenchChampionClickable = (champ: any) => {
  if (!isPickable(champ.championId)) return false;

  // 自己的随时可以点回，不作保护期限制
  if (champ.isMine === true) return true;

  // 倒计时 timer.phase === "FINALIZATION" 是最后的全面开放阶段，肯定是可以选的
  if (store.champSelectSession?.timer?.phase === "FINALIZATION") {
    return true;
  }

  const addedTime = benchChampionsAddedTime.value[champ.championId] || sessionStartTimestamp.value;
  const elapsed = Date.now() - addedTime;
  const elapsedFromStart = Date.now() - sessionStartTimestamp.value;

  // 1. 刚开局前 5 秒内，大家的英雄都还在加载或随机，非本人的英雄直接禁用
  if (elapsedFromStart < 5000) {
    return false;
  }

  // 2. 新放到板凳席不足 3.5 秒的，还在队友保护冷却期内，不能选择
  if (elapsed < 3500) {
    return false;
  }

  // 其它情况（过了保护期），所有人均可自由交换
  return true;
};

// 获取不可选原因的提示文案
const getDisabledReason = (champ: any) => {
  if (!isPickable(champ.championId)) return "未拥有/不可用";
  if (champ.isMine === true) return "点击秒抢";

  if (store.champSelectSession?.timer?.phase !== "FINALIZATION") {
    const addedTime = benchChampionsAddedTime.value[champ.championId] || sessionStartTimestamp.value;
    const elapsed = Date.now() - addedTime;
    const elapsedFromStart = Date.now() - sessionStartTimestamp.value;

    if (elapsedFromStart < 5000) {
      return "开局准备中，请稍候";
    }
    if (elapsed < 3500) {
      return "保护期内 (队友优先)";
    }
  }
  return "点击秒抢";
};

// 点击选择/抢下板凳席英雄
async function swapChampion(champ: any) {
  if (!isPickable(champ.championId)) {
    console.warn(
      `[BenchOverlay] 拦截点击：英雄 ${champ.championId} 不在玩家当前可用池内`,
    );
    return;
  }
  
  if (champ.isMine !== true) {
    const addedTime = benchChampionsAddedTime.value[champ.championId] || sessionStartTimestamp.value;
    const elapsed = Date.now() - addedTime;
    const elapsedFromStart = Date.now() - sessionStartTimestamp.value;
    const isFinalization = store.champSelectSession?.timer?.phase === "FINALIZATION";

    if (!isFinalization) {
      if (elapsedFromStart < 5000) {
        console.warn(`[BenchOverlay] 拦截点击：开局前5秒保护中`);
        return;
      }
      if (elapsed < 3500) {
        console.warn(
          `[BenchOverlay] 拦截点击：英雄 ${champ.championId} 处于队友保护期内`,
        );
        return;
      }
    }
  }

  console.log(
    `[BenchOverlay] 请求交换: championId=${champ.championId} isMine=${champ.isMine} phase=${store.champSelectSession?.timer?.phase}`,
  );
  const resp = await lcuRequest(
    "POST",
    `/lol-champ-select/v1/session/bench/swap/${champ.championId}`,
    {},
  );
  if (resp.success) {
    console.log(`[BenchOverlay] 抢英雄成功: ${champ.championId}`);
  } else {
    console.error(`[BenchOverlay] 抢英雄失败: ${champ.championId}, 错误:`, resp.error);
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
  (phase) => {
    if (phase === "ChampSelect") {
      sessionStartTimestamp.value = Date.now();
      benchChampionsAddedTime.value = {};
      console.log("[BenchOverlay] 进入选人阶段，初始化开始时间戳和板凳席时间映射");
    }
  },
  { immediate: true }
);

watch(
  () => store.champSelectSession,
  async (session) => {
    console.log("[BenchOverlay] LCU Session 改变:", session);
    if (session) {
      // 智能初始化 sessionStartTimestamp，防止中途打开悬浮窗时误判为“刚开局”
      const timeLeft = session.timer?.adjustedTimeLeftInPhase ?? 0;
      if (timeLeft > 0 && timeLeft < 53000 && session.timer?.phase === "PLANNING") {
        if (Date.now() - sessionStartTimestamp.value < 5000) {
          sessionStartTimestamp.value = Date.now() - 10000;
          console.log("[BenchOverlay] 检测到中途打开悬浮窗，已跳过开局保护时间");
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

      const hasMissingIsMine = session.benchChampions?.some(
        (c: any) => c && c.isMine === undefined,
      );
      if (hasMissingIsMine) {
        console.log("[BenchOverlay] 检测到 isMine 缺失，从 REST API 补充拉取 session...");
        refreshSessionIsMine();
      }

      // 更新板凳席英雄加入的时间戳
      const now = Date.now();
      const newChampions = session.benchChampions || [];
      const currentIds = newChampions.map((c: any) => c?.championId).filter(Boolean);

      // 移除已经不在板凳席的英雄
      for (const id of Object.keys(benchChampionsAddedTime.value)) {
        if (!currentIds.includes(Number(id))) {
          delete benchChampionsAddedTime.value[Number(id)];
        }
      }

      // 添加新加入板凳席的英雄
      for (const c of newChampions) {
        if (c && c.championId) {
          if (benchChampionsAddedTime.value[c.championId] === undefined) {
            const isFreshSession = now - sessionStartTimestamp.value < 5000;
            benchChampionsAddedTime.value[c.championId] = isFreshSession ? sessionStartTimestamp.value : now;
            console.log(`[BenchOverlay] 英雄 ${c.championId} 录入板凳席，设定加入时间为: ${benchChampionsAddedTime.value[c.championId]}`);
          }
        }
      }
    }
  },
  { immediate: true },
);

/** 当 WS 推送数据缺少 isMine 时，从 REST API 重试拉取完整 session（延迟递增） */
async function refreshSessionIsMine(maxRetries = 8) {
  for (let i = 0; i < maxRetries; i++) {
    if (!store.champSelectSession?.benchChampions?.some(
      (c: any) => c && c.isMine === undefined,
    )) {
      console.log("[BenchOverlay] isMine 已由后续 WS 推送补全，不再重试");
      return;
    }
    const resp = await lcuRequest<any>(
      "GET",
      "/lol-champ-select/v1/session",
    );
    if (resp.success && resp.data?.benchChampions?.length) {
      const restHasMine = resp.data.benchChampions.some(
        (c: any) => c && c.isMine !== undefined,
      );
      if (restHasMine) {
        console.log(`[BenchOverlay] REST 数据包含 isMine，覆盖 store（重试 #${i + 1}）`);
        store.setChampSelectSession(resp.data);
        return;
      }
    }
    const delay = Math.min(500 + i * 500, 3000);
    console.log(`[BenchOverlay] REST 也没有 isMine，${delay}ms 后重试...`);
    await new Promise((r) => setTimeout(r, delay));
  }
  console.warn("[BenchOverlay] 重试结束，仍未获取到 isMine，可能 LCU 接口暂无此字段");
}
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
  </div>
</template>

<style scoped>
.bench-overlay-wrapper {
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
</style>
