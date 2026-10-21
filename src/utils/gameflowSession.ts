import { lcuRequest } from "../api/lcu";
import type { GameflowSession } from "../types/lcu";

// ── gameflow session 短期缓存，避免同一流程中多次请求同一端点
let cachedSession: { data: GameflowSession; timestamp: number } | null = null;
const SESSION_CACHE_TTL = 30 * 1000; // 30 秒

export async function fetchSessionCached(): Promise<GameflowSession | null> {
  const now = Date.now();
  if (cachedSession && now - cachedSession.timestamp < SESSION_CACHE_TTL) {
    return cachedSession.data;
  }
  try {
    const resp = await lcuRequest<GameflowSession>(
      "GET",
      "/lol-gameflow/v1/session",
    );
    if (resp.success && resp.data) {
      cachedSession = { data: resp.data, timestamp: now };
      return resp.data;
    }
  } catch {
    /* ignore */
  }
  return null;
}

/** 主动失效缓存：会话状态切换（进游戏/退出对局）后必须丢弃旧快照 */
export function clearSessionCache(): void {
  cachedSession = null;
}

/** 用实时事件推送的 session 覆盖缓存，省掉一次轮询请求 */
export function seedSessionCache(session: GameflowSession): void {
  cachedSession = { data: session, timestamp: Date.now() };
}
