import ironMedal from "../assets/ranked-icons/iron.png";
import bronzeMedal from "../assets/ranked-icons/bronze.png";
import silverMedal from "../assets/ranked-icons/silver.png";
import goldMedal from "../assets/ranked-icons/gold.png";
import platinumMedal from "../assets/ranked-icons/platinum.png";
import emeraldMedal from "../assets/ranked-icons/emerald.png";
import diamondMedal from "../assets/ranked-icons/diamond.png";
import masterMedal from "../assets/ranked-icons/master.png";
import grandmasterMedal from "../assets/ranked-icons/grandmaster.png";
import challengerMedal from "../assets/ranked-icons/challenger.png";

import ironLarge from "../assets/ranked-icons-large/iron.png";
import bronzeLarge from "../assets/ranked-icons-large/bronze.png";
import silverLarge from "../assets/ranked-icons-large/silver.png";
import goldLarge from "../assets/ranked-icons-large/gold.png";
import platinumLarge from "../assets/ranked-icons-large/platinum.png";
import emeraldLarge from "../assets/ranked-icons-large/emerald.png";
import diamondLarge from "../assets/ranked-icons-large/diamond.png";
import masterLarge from "../assets/ranked-icons-large/master.png";
import grandmasterLarge from "../assets/ranked-icons-large/grandmaster.png";
import challengerLarge from "../assets/ranked-icons-large/challenger.png";
import unrankedLarge from "../assets/ranked-icons-large/unranked.png";

export const RANKED_MEDAL_ICONS: Record<string, string> = {
  IRON: ironMedal,
  BRONZE: bronzeMedal,
  SILVER: silverMedal,
  GOLD: goldMedal,
  PLATINUM: platinumMedal,
  EMERALD: emeraldMedal,
  DIAMOND: diamondMedal,
  MASTER: masterMedal,
  GRANDMASTER: grandmasterMedal,
  CHALLENGER: challengerMedal,
};

export const RANKED_LARGE_ICONS: Record<string, string> = {
  UNRANKED: unrankedLarge,
  IRON: ironLarge,
  BRONZE: bronzeLarge,
  SILVER: silverLarge,
  GOLD: goldLarge,
  PLATINUM: platinumLarge,
  EMERALD: emeraldLarge,
  DIAMOND: diamondLarge,
  MASTER: masterLarge,
  GRANDMASTER: grandmasterLarge,
  CHALLENGER: challengerLarge,
};

export const RANKED_TIER_NAMES: Record<string, string> = {
  NONE: "未定级",
  UNRANKED: "未定级",
  IRON: "坚韧黑铁",
  BRONZE: "英勇黄铜",
  SILVER: "不屈白银",
  GOLD: "荣耀黄金",
  PLATINUM: "华贵铂金",
  EMERALD: "流光翡翠",
  DIAMOND: "璀璨钻石",
  MASTER: "超凡大师",
  GRANDMASTER: "傲世宗师",
  CHALLENGER: "最强王者",
};

export const RANKED_TIER_SHORT_NAMES: Record<string, string> = {
  NONE: "无",
  UNRANKED: "无",
  IRON: "黑铁",
  BRONZE: "黄铜",
  SILVER: "白银",
  GOLD: "黄金",
  PLATINUM: "铂金",
  EMERALD: "翡翠",
  DIAMOND: "钻石",
  MASTER: "大师",
  GRANDMASTER: "宗师",
  CHALLENGER: "王者",
};

export function getRankedTierShortName(tier?: string | null): string {
  if (!tier) return "";
  const upper = tier.toUpperCase();
  return RANKED_TIER_SHORT_NAMES[upper] || tier;
}

export const RANKED_TIER_ORDER: Record<string, number> = {
  UNRANKED: 0,
  NONE: 0,
  NA: 0,
  IRON: 1,
  BRONZE: 2,
  SILVER: 3,
  GOLD: 4,
  PLATINUM: 5,
  EMERALD: 6,
  DIAMOND: 7,
  MASTER: 8,
  GRANDMASTER: 9,
  CHALLENGER: 10,
};

/**
 * 获取段位小勋章图标路径 (16x16 / 20x20 勋章)
 */
export function getRankedMedalIcon(tier?: string | null): string | null {
  if (!tier) return null;
  const key = tier.trim().toUpperCase();
  return RANKED_MEDAL_ICONS[key] || null;
}

/**
 * 获取段位大徽章图标路径 (大立绘)
 */
export function getRankedLargeIcon(tier?: string | null): string | null {
  if (!tier) return RANKED_LARGE_ICONS.UNRANKED;
  const key = tier.trim().toUpperCase();
  return RANKED_LARGE_ICONS[key] || RANKED_LARGE_ICONS.UNRANKED;
}
