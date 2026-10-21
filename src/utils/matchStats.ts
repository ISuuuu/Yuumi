import type { MatchDisplay } from "../api/lcu";
import type { StreakInfo } from "../types/gameInfo";
import {
  computeAggregatedPositions,
  computeAggregatedChampions,
} from "./matchAnalysis";

/** 由对局列表计算胜率/KDA/连胜/位置与英雄聚合（供首屏 10 局与后台 40 局共用） */
export function computeMatchStats(matches: MatchDisplay[]) {
  let totalKills = 0;
  let totalDeaths = 0;
  let totalAssists = 0;
  let remakeCount = 0;
  let currentWinCount = 0;
  let currentLossesCount = 0;

  matches.forEach((m: MatchDisplay) => {
    if (m.remake) {
      remakeCount++;
    } else {
      totalKills += m.kills ?? 0;
      totalDeaths += m.deaths ?? 0;
      totalAssists += m.assists ?? 0;
      if (m.win) {
        currentWinCount++;
      } else {
        currentLossesCount++;
      }
    }
  });

  const validMatches = matches.length - remakeCount;
  const winRate =
    validMatches > 0 ? Math.round((currentWinCount / validMatches) * 100) : 0;
  const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
  const avgKda = (totalKills + totalAssists) / deathsForCalc;

  let streak: StreakInfo | null = null;
  const validStreakMatches = matches.filter((m) => !m.remake);
  if (validStreakMatches.length > 0) {
    const firstWin = validStreakMatches[0].win;
    let count = 0;
    for (const m of validStreakMatches) {
      if (m.win === firstWin) {
        count++;
      } else {
        break;
      }
    }
    if (count >= 2) {
      streak = {
        type: firstWin ? "win" : "loss",
        count,
      };
    }
  }

  const positions = computeAggregatedPositions(matches);
  const topChampions = computeAggregatedChampions(matches, 5);

  return {
    winCount: currentWinCount,
    lossesCount: currentLossesCount,
    winRate,
    avgKda,
    streak,
    positions,
    topChampions,
  };
}
