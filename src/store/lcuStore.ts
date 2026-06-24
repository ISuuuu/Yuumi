import { create } from "zustand";
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

interface LcuState {
  /** LCU 是否已连接 */
  isConnected: boolean;
  /** WebSocket 是否已连接 */
  wsConnected: boolean;
  /** 当前游戏阶段 */
  gamePhase: GamePhase;
  /** 选人会话数据 */
  champSelectSession: any | null;
  /** 匹配准备检查数据 */
  readyCheck: any | null;

  setConnected: (connected: boolean) => void;
  setWsConnected: (connected: boolean) => void;
  setGamePhase: (phase: GamePhase) => void;
  setChampSelectSession: (session: any | null) => void;
  setReadyCheck: (data: any | null) => void;
}

export const useLcuStore = create<LcuState>((set) => ({
  isConnected: false,
  wsConnected: false,
  gamePhase: "None",
  champSelectSession: null,
  readyCheck: null,

  setConnected: (connected) => set({ isConnected: connected }),
  setWsConnected: (connected) => set({ wsConnected: connected }),
  setGamePhase: (phase) => set({ gamePhase: phase }),
  setChampSelectSession: (session) => set({ champSelectSession: session }),
  setReadyCheck: (data) => set({ readyCheck: data }),
}));

/**
 * 初始化全局 LCU 事件监听。
 * 在应用启动时调用一次，将 Tauri 事件映射到 Zustand store。
 */
export function initLcuListeners() {
  // LCU 客户端启动/关闭
  listen("lcu-client-started", () => {
    useLcuStore.getState().setConnected(true);
  });

  listen("lcu-client-ended", () => {
    useLcuStore.getState().setConnected(false);
    useLcuStore.getState().setWsConnected(false);
    useLcuStore.getState().setGamePhase("None");
    useLcuStore.getState().setChampSelectSession(null);
    useLcuStore.getState().setReadyCheck(null);
  });

  // WebSocket 连接状态
  listen("lcu-ws-connected", () => {
    useLcuStore.getState().setWsConnected(true);
  });

  listen("lcu-ws-disconnected", () => {
    useLcuStore.getState().setWsConnected(false);
  });

  // WebSocket 事件分发
  listen<any>("lcu-ws-event", (event) => {
    const payload = event.payload;
    const uri: string = payload?.uri ?? "";
    const data = payload?.data;

    if (uri.startsWith("/lol-gameflow/v1/gameflow-phase")) {
      useLcuStore.getState().setGamePhase(data);
    } else if (uri.startsWith("/lol-champ-select/v1/session")) {
      useLcuStore.getState().setChampSelectSession(data);
    } else if (uri.startsWith("/lol-matchmaking/v1/ready-check")) {
      useLcuStore.getState().setReadyCheck(data);
    }
  });
}
