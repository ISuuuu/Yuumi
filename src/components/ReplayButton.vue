<script setup lang="ts">
import { ref, onUnmounted } from "vue";
import { downloadGameReplay, getReplayStatus, watchGameReplay } from "../api/lcu";
import { useToast } from "../composables/useToast";

const props = withDefaults(
  defineProps<{
    gameId: number;
    compact?: boolean;
  }>(),
  {
    compact: false,
  },
);

const { showToast } = useToast();

type ReplayState = "idle" | "checking" | "downloading" | "watch" | "unsupported" | "error";

const state = ref<ReplayState>("idle");
let pollTimer: ReturnType<typeof setInterval> | null = null;
let pollCount = 0;
const MAX_POLLS = 40; // 40 * 1.5s = 60s 超时

function clearPoll() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

onUnmounted(() => {
  clearPoll();
});

function startPolling() {
  clearPoll();
  pollCount = 0;
  pollTimer = setInterval(async () => {
    pollCount++;
    if (pollCount > MAX_POLLS) {
      clearPoll();
      if (state.value === "downloading" || state.value === "checking") {
        state.value = "idle";
        showToast("下载回放超时，请重试", "warning");
      }
      return;
    }

    try {
      const current = await getReplayStatus(props.gameId);
      const s = (current || "").toLowerCase();
      if (s === "watch") {
        clearPoll();
        state.value = "watch";
        showToast("回放下载完成，可点击观看", "success");
      } else if (s === "unsupported") {
        clearPoll();
        state.value = "unsupported";
        showToast("该对局回放版本已过期或不受支持", "warning");
      } else if (s === "downloading" || s === "checking") {
        state.value = "downloading";
      }
    } catch {
      // 忽略单次轮询异常，继续等待
    }
  }, 1500);
}

async function handleClick(e: MouseEvent) {
  e.stopPropagation();

  if (state.value === "downloading" || state.value === "checking") {
    showToast("回放正在下载中，请稍候...", "info");
    return;
  }

  if (state.value === "unsupported") {
    showToast("该对局回放已过期或不可用", "warning");
    return;
  }

  if (state.value === "watch") {
    try {
      showToast("正在启动对局回放...", "info");
      await watchGameReplay(props.gameId);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      showToast(`启动回放失败: ${msg}`, "error");
    }
    return;
  }

  // idle 或 error 状态下触发
  try {
    state.value = "checking";
    const curStatus = await getReplayStatus(props.gameId);
    const s = (curStatus || "").toLowerCase();

    if (s === "watch") {
      state.value = "watch";
      showToast("回放已下载就绪，正在启动...", "info");
      try {
        await watchGameReplay(props.gameId);
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        showToast(`启动回放失败: ${msg}`, "error");
      }
      return;
    }

    if (s === "unsupported") {
      state.value = "unsupported";
      showToast("该对局回放已过期或不受支持", "warning");
      return;
    }

    // 需要下载
    state.value = "downloading";
    await downloadGameReplay(props.gameId);
    showToast("已开始下载回放文件...", "info");
    startPolling();
  } catch (err: unknown) {
    state.value = "error";
    const msg = err instanceof Error ? err.message : String(err);
    showToast(`获取回放失败: ${msg}`, "error");
  }
}
</script>

<template>
  <button
    class="replay-btn"
    :class="[
      state,
      { compact },
    ]"
    :title="
      state === 'watch'
        ? '观看回放'
        : state === 'downloading'
          ? '回放下载中...'
          : state === 'unsupported'
            ? '回放不可用'
            : '下载并观看对局回放'
    "
    @click="handleClick"
  >
    <!-- 下载中 Loading Spinner -->
    <svg
      v-if="state === 'downloading' || state === 'checking'"
      class="replay-icon spin"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <line x1="12" y1="2" x2="12" y2="6"></line>
      <line x1="12" y1="18" x2="12" y2="22"></line>
      <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
      <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
      <line x1="2" y1="12" x2="6" y2="12"></line>
      <line x1="18" y1="12" x2="22" y2="12"></line>
      <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"></line>
      <line x1="16.24" y1="7.76" x2="19.07" y2="4.93"></line>
    </svg>

    <!-- 已就绪可观看 Play 图标 -->
    <svg
      v-else-if="state === 'watch'"
      class="replay-icon watch-icon"
      viewBox="0 0 24 24"
      fill="currentColor"
    >
      <polygon points="5 3 19 12 5 21 5 3"></polygon>
    </svg>

    <!-- 不可用/已过期图标 -->
    <svg
      v-else-if="state === 'unsupported'"
      class="replay-icon disabled-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <circle cx="12" cy="12" r="10"></circle>
      <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"></line>
    </svg>

    <!-- 初始/下载图标 -->
    <svg
      v-else
      class="replay-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
      <polyline points="7 10 12 15 17 10"></polyline>
      <line x1="12" y1="15" x2="12" y2="3"></line>
    </svg>

    <!-- 文字标签（非紧凑模式） -->
    <span v-if="!compact" class="replay-label">
      {{
        state === "watch"
          ? "观看回放"
          : state === "downloading"
            ? "下载中..."
            : state === "checking"
              ? "检查中..."
              : state === "unsupported"
                ? "不可用"
                : "回放"
      }}
    </span>
  </button>
</template>

<style scoped>
.replay-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  color: var(--text-muted);
  font-size: 0.76rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  white-space: nowrap;
  backdrop-filter: blur(8px);
  outline: none;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
}

.replay-btn:hover {
  transform: translateY(-1px);
  color: var(--primary-color, #0ea5e9);
  border-color: var(--primary-color, #0ea5e9);
  box-shadow: 0 3px 8px rgba(14, 165, 233, 0.15);
}

.replay-btn.compact {
  padding: 4px;
  border-radius: 5px;
  min-width: 24px;
  height: 24px;
}

.replay-icon {
  width: 13px;
  height: 13px;
  flex-shrink: 0;
}

.replay-btn.watch {
  background: rgba(34, 197, 94, 0.1);
  border-color: rgba(34, 197, 94, 0.35);
  color: #16a34a;
}

.replay-btn.watch:hover {
  background: rgba(34, 197, 94, 0.2);
  border-color: #16a34a;
  box-shadow: 0 3px 8px rgba(34, 197, 94, 0.2);
}

.replay-btn.downloading,
.replay-btn.checking {
  background: rgba(14, 165, 233, 0.1);
  border-color: rgba(14, 165, 233, 0.35);
  color: #0ea5e9;
  cursor: wait;
}

.replay-btn.unsupported {
  opacity: 0.5;
  cursor: not-allowed;
  border-color: transparent;
  background: rgba(0, 0, 0, 0.03);
}

.replay-btn.unsupported:hover {
  transform: none;
  box-shadow: none;
  border-color: transparent;
  color: var(--text-dimmed);
}

.spin {
  animation: replay-spin 1.2s linear infinite;
}

@keyframes replay-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
