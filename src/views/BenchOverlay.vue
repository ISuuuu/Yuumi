<script setup lang="ts">
import { ref, onMounted, computed, watch } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { lcuRequest, fetchConfig } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

const store = useLcuStore();
const pickableIds = ref<number[]>([]);

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
    // 如果是 404 或无活动代理错误（代表没在选人阶段），直接终止重试，静默返回
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
      // 检查系统暗色偏好
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
  if (pickableIds.value.length === 0) return true; // 兜底：如果列表为空，默认允许，防止接口返回延迟或异常导致全不可选
  return pickableIds.value.includes(championId);
};

// 点击选择/抢下板凳席英雄
async function swapChampion(championId: number) {
  if (!isPickable(championId)) {
    console.warn(
      `[BenchOverlay] 拦截点击：英雄 ${championId} 不在玩家当前可用池内`,
    );
    return;
  }
  console.log(`[BenchOverlay] 尝试兑换板凳席英雄: ${championId}`);
  const resp = await lcuRequest(
    "POST",
    `/lol-champ-select/v1/session/bench/swap/${championId}`,
    {},
  );
  if (resp.success) {
    console.log(`[BenchOverlay] 抢英雄成功: ${championId}`);
  } else {
    console.error(
      `[BenchOverlay] 抢英雄失败: ${championId}, 错误:`,
      resp.error,
    );
  }
}

// 关闭悬浮窗
function closeOverlay() {
  getCurrentWindow().close();
}

// 开始拖动窗口（JS 兜底方案，避开 Tauri 2 原生 drag-region 的平台兼容性 Bug）
function startDrag() {
  getCurrentWindow()
    .startDragging()
    .catch((err) => {
      console.warn("[BenchOverlay] 启动拖动失败:", err);
    });
}

watch(
  () => store.champSelectSession,
  (session) => {
    console.log("[BenchOverlay] LCU Session 改变:", session);
    if (session) {
      console.log(
        "[BenchOverlay] 板凳席状态: benchEnabled =",
        session.benchEnabled,
        "benchChampions =",
        session.benchChampions,
      );
      // 如果可用英雄列表仍为空，则通过更新 session 触发拉取（不依赖 gamePhase 状态同步）
      if (pickableIds.value.length === 0) {
        console.log("[BenchOverlay] 选人会话更新且可用列表为空，补充拉取...");
        fetchPickableIds();
      }
    }
  },
  { deep: true, immediate: true },
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
        :class="['champ-item', { disabled: !isPickable(champ.championId) }]"
        @click="swapChampion(champ.championId)"
        :title="isPickable(champ.championId) ? '点击秒抢' : '未拥有/不可用'"
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
