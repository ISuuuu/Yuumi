import { defineStore } from "pinia";
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";

export type GamePhase =
  | "None"
  | "Lobby"
  | "Matchmaking"
  | "ReadyCheck"
  | "ChampSelect"
  | "InProgress"
  | "EndOfGame"
  | string;

export const useLcuStore = defineStore("lcu", () => {
  const isConnected = ref(false);
  const wsConnected = ref(false);
  const gamePhase = ref<GamePhase>("None");
  const champSelectSession = ref<any | null>(null);
  const readyCheck = ref<any | null>(null);

  function setConnected(v: boolean) {
    isConnected.value = v;
  }
  function setWsConnected(v: boolean) {
    wsConnected.value = v;
  }
  function setGamePhase(v: GamePhase) {
    gamePhase.value = v;
  }
  function setChampSelectSession(v: any | null) {
    champSelectSession.value = v;
  }
  function setReadyCheck(v: any | null) {
    readyCheck.value = v;
  }

  return {
    isConnected,
    wsConnected,
    gamePhase,
    champSelectSession,
    readyCheck,
    setConnected,
    setWsConnected,
    setGamePhase,
    setChampSelectSession,
    setReadyCheck,
  };
});

/**
 * 初始化全局 LCU 事件监听。
 * 在应用启动时调用一次，将 Tauri 事件映射到 Pinia store。
 */
export function initLcuListeners() {
  const store = useLcuStore();

  listen("lcu-client-started", () => {
    store.setConnected(true);
  });

  listen("lcu-client-ended", () => {
    store.setConnected(false);
    store.setWsConnected(false);
    store.setGamePhase("None");
    store.setChampSelectSession(null);
    store.setReadyCheck(null);
  });

  listen("lcu-ws-connected", () => {
    store.setWsConnected(true);
  });

  listen("lcu-ws-disconnected", () => {
    store.setWsConnected(false);
  });

  listen<any>("lcu-ws-event", (event) => {
    const payload = event.payload;
    const uri: string = payload?.uri ?? "";
    const data = payload?.data;

    if (uri.startsWith("/lol-gameflow/v1/gameflow-phase")) {
      store.setGamePhase(data);
    } else if (uri.startsWith("/lol-champ-select/v1/session")) {
      store.setChampSelectSession(data);
    } else if (uri.startsWith("/lol-matchmaking/v1/ready-check")) {
      store.setReadyCheck(data);
    }
  });
}
