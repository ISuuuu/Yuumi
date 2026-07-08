import { defineStore } from "pinia";
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type GamePhase =
  | "None"
  | "Lobby"
  | "Matchmaking"
  | "ReadyCheck"
  | "ChampSelect"
  | "GameStart"
  | "InProgress"
  | "EndOfGame"
  | string;

export const useLcuStore = defineStore("lcu", () => {
  const isConnected = ref(false);
  const wsConnected = ref(false);
  const gamePhase = ref<GamePhase>("None");
  const champSelectSession = ref<any | null>(null);
  const readyCheck = ref<any | null>(null);
  // lcu-client-started 事件计数器，用于触发 App.vue 重新加载状态
  const connectionVersion = ref(0);

  // 共享的状态用于页面跳转和跨页面数据传递
  const currentPage = ref("home");
  const searchQuery = ref("");
  const selectedGameId = ref<number | null>(null);

  function setConnected(v: boolean) {
    isConnected.value = v;
  }
  function setWsConnected(v: boolean) {
    wsConnected.value = v;
  }
  function setGamePhase(v: GamePhase) {
    console.log("[lcuStore] setGamePhase:", v, "prev:", gamePhase.value);
    gamePhase.value = v;
  }
  function setChampSelectSession(v: any | null) {
    champSelectSession.value = v;
  }
  function setReadyCheck(v: any | null) {
    readyCheck.value = v;
  }
  function setCurrentPage(v: string) {
    currentPage.value = v;
  }
  function setSearchQuery(v: string) {
    searchQuery.value = v;
  }
  function setSelectedGameId(v: number | null) {
    selectedGameId.value = v;
  }

  return {
    isConnected,
    wsConnected,
    gamePhase,
    champSelectSession,
    readyCheck,
    connectionVersion,
    currentPage,
    searchQuery,
    selectedGameId,
    setConnected,
    setWsConnected,
    setGamePhase,
    setChampSelectSession,
    setReadyCheck,
    setCurrentPage,
    setSearchQuery,
    setSelectedGameId,
  };
});

// 防止重复注册
let _listenersInitialized = false;

/**
 * 初始化全局 LCU 事件监听。
 * 在应用启动时调用一次，将 Tauri 事件映射到 Pinia store。
 */
export async function initLcuListeners() {
  if (_listenersInitialized) {
    console.warn("[lcuStore] initLcuListeners already called, skipping");
    return;
  }
  _listenersInitialized = true;

  const store = useLcuStore();

  // 必须 await listen()，否则监听器可能还未注册就开始接收事件
  await listen("lcu-client-started", () => {
    console.log("[lcuStore] lcu-client-started");
    store.setConnected(true);
    store.connectionVersion++;
  });

  await listen("lcu-client-ended", () => {
    console.log("[lcuStore] lcu-client-ended");
    store.setConnected(false);
    store.setWsConnected(false);
    store.setGamePhase("None");
    store.setChampSelectSession(null);
    store.setReadyCheck(null);
  });

  await listen("lcu-ws-connected", () => {
    console.log("[lcuStore] lcu-ws-connected");
    store.setWsConnected(true);
  });

  await listen("lcu-ws-disconnected", () => {
    console.log("[lcuStore] lcu-ws-disconnected");
    store.setWsConnected(false);
  });

  // Rust 侧 WS 连接失败时的错误信息（对应 try_connect 返回 Err）
  await listen<string>("lcu-ws-error", (event) => {
    console.error("[lcuStore] lcu-ws-error:", event.payload);
  });

  await listen<any>("lcu-ws-event", (event) => {
    const payload = event.payload;
    const uri: string = payload?.uri ?? "";
    const data = payload?.data;

    console.log("[lcuStore] lcu-ws-event uri:", uri, "data:", data);

    if (uri.startsWith("/lol-gameflow/v1/gameflow-phase")) {
      store.setGamePhase(data);
    } else if (uri.startsWith("/lol-champ-select/v1/session")) {
      store.setChampSelectSession(data);
    } else if (uri.startsWith("/lol-matchmaking/v1/ready-check")) {
      store.setReadyCheck(data);
    }
  });

  // 对局结束自动上传成功事件（由 Rust UploadQueue worker 触发）
  await listen<{ gameId: number }>("upload-success", (event) => {
    console.log(`[lcuStore] upload-success: gameId=${event.payload.gameId}`);
  });

  console.log("[lcuStore] all listeners registered");

  // 页面刷新后从后端同步当前连接状态（后端 AppState 持久，前端 store 会丢失）
  try {
    const info = await invoke<{
      pid: number;
      port: number;
      token: string;
    } | null>("get_lcu_connection_info");
    if (info && info.pid > 0) {
      console.log(
        "[lcuStore] 从后端恢复连接状态: pid=",
        info.pid,
        "port=",
        info.port,
      );
      store.setConnected(true);
    }
  } catch (e) {
    console.warn("[lcuStore] 同步连接状态失败:", e);
  }
}
