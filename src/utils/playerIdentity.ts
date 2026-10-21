import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";

// ── 槽位英雄 ID 继承：保留已有英雄 ID，只有条目与 incoming 身份明确冲突时才严禁串用
export function inheritPlaceholderChampion(
  entry: PlayerData | undefined,
  cellId: number,
  incoming?: { puuid?: string; summonerId?: number },
): number {
  const champ = entry?.championId ?? 0;
  if (!champ || champ <= 0) return 0;
  if (incoming) {
    const inPuuid = (incoming.puuid || "").trim();
    const inSid = incoming.summonerId || 0;
    const inSidReal = Boolean(inSid) && inSid !== cellId;
    const ePuuid = (entry?.info?.puuid || "").trim();
    const eRawSid = entry?.info?.summonerId || 0;
    const eSid = eRawSid === cellId ? 0 : eRawSid;
    // 仅在明确与不同真实身份冲突时拒绝继承
    if (inPuuid && ePuuid && inPuuid !== ePuuid) return 0;
    if (inSidReal && eSid && inSid !== eSid) return 0;
  }
  return champ;
}

// teamParticipantId / partyId 有效值提取：0、"0"、""、NaN 等空值视为无效，
// 优先 teamParticipantId，其次 partyId（组队标识迁移/继承多处共用）
export function getValidTpid(
  p?: PremadePlayerLike | null,
): number | string | undefined {
  const tpid = p?.teamParticipantId;
  if (tpid && tpid !== "0") return tpid;
  const partyId = p?.partyId;
  if (partyId && partyId !== "0") return partyId;
  return undefined;
}

// ── 新号判定上限：空战绩 + 等级在此之下视为从未打过的新号，不标隐藏
export const NEW_PLAYER_MAX_LEVEL = 30;

// ── 身份门禁（canonical）：incoming 的真实身份键与条目是否冲突。
// puuid 非空即真实；summonerId 仅在非 0 且不等于 cell 槽位时视为真实
// （选人/对局切换时 cellId 会被充作 sid 兜底，不可参与比对）。
// 条目侧同理：info 缺失（loading 占位）或 sid 恰为 cell 槽位都视为无身份。
// 严防脱敏匿名玩家复用已有实名玩家的数据（消除 ChampSelect 限制，任何阶段严格生效）：
// 任何阶段无实名身份槽位严禁继承有实名身份旧数据，反之亦然！
export function isIdentityCompatible(
  entry: PlayerData | undefined,
  incoming: { puuid?: string; summonerId?: number; cellId?: number },
): boolean {
  if (!entry?.info) return false;
  const cell = incoming.cellId ?? -1;
  const inPuuid = (incoming.puuid || "").trim();
  const inSid = incoming.summonerId || 0;
  const inSidReal = Boolean(inSid) && inSid !== cell;
  const ePuuid = (entry.info.puuid || "").trim();
  const eRawSid = entry.info.summonerId || 0;
  const eSid = eRawSid === cell ? 0 : eRawSid;

  if (inPuuid && ePuuid && inPuuid !== ePuuid) return false;
  if (inSidReal && eSid && inSid !== eSid) return false;

  // 严防脱敏匿名玩家复用已有实名玩家的数据（消除 ChampSelect 限制，任何阶段生效）
  // 任何一方有实名身份而另一方无实名身份，均不兼容
  const incomingHasIdentity = Boolean(inPuuid || inSidReal);
  const entryHasIdentity = Boolean(ePuuid || eSid);
  if (incomingHasIdentity !== entryHasIdentity) {
    return false;
  }

  return true;
}
