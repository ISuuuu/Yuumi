<script setup lang="ts">
import { useI18n } from "vue-i18n";
import LcuImage from "../LcuImage.vue";
import type { GameDetail } from "../../types/search";
import { getQueueName } from "../../utils/queueName";

defineProps<{
  details: GameDetail | null;
  loading: boolean;
  queueId: number | null;
  participantRanks: Record<string, string>;
  myPuuid?: string;
}>();

const emit = defineEmits<{
  (e: "copy", gameId: number): void;
  (e: "search-player", summonerId: number, name: string): void;
}>();

const { t, te } = useI18n();

function queueName(queueId: number, backendName: string): string {
  return getQueueName(queueId, backendName, { t, te });
}
</script>

<template>
  <div class="right-detail-panel">
    <div v-if="loading && !details" class="detail-loading">
      <div class="loading-spinner"></div>
    </div>

    <div v-show="details" class="detail-content">
      <!-- 头部大 Banner -->
      <div v-if="details" :class="['detail-banner', details.win ? 'win' : 'lose']">
        <div class="banner-main">
          <div class="banner-map-icon">
            <img :src="details.mapIconUrl" alt="map" />
          </div>
          <div class="banner-left">
            <h2 :class="['banner-result', details.win ? 'win' : 'lose']">
              {{ details.win ? $t("career.victory") : $t("career.defeat") }}
            </h2>
            <span class="banner-subtext">
              {{ $t("maps." + details.mapId) }} ·
              {{ queueName(details.queueId, details.queueName) }}
              · {{ details.duration }} · {{ details.date }} ·
              {{ $t("career.gameId") || "Game ID" }}:
              {{ details.gameId }}
            </span>
          </div>
        </div>
        <button
          class="copy-btn"
          title="复制游戏 ID"
          @click="emit('copy', details.gameId)"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path
              d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
            />
          </svg>
        </button>
      </div>

      <!-- 队伍详细数据 -->
      <div v-if="details" class="teams-container">
        <!-- 胜方 / 败方 -->
        <div
          v-for="team in [details.blue, details.red]"
          :key="team.win ? 'win' : 'lose'"
          :class="['team-block', team.win ? 'win-block' : 'lose-block']"
        >
          <!-- 队头资源概览 -->
          <div
            :class="['team-header-bar', team.win ? 'win-bar' : 'lose-bar']"
          >
            <span
              :class="['team-result-label', team.win ? 'win-text' : 'lose-text']"
            >
              {{ team.win ? "胜方" : "败方" }}
            </span>

            <div class="team-objectives">
              <span class="obj-item" title="击杀"
                ><img class="obj-icon-img" src="/images/kills.png" /> {{ team.kills }}</span
              >
              <span class="obj-item" title="摧毁防御塔"
                ><img class="obj-icon-img" :src="`/images/tower-${team.teamId}.png`" />
                {{ team.towerKills }}</span
              >
              <span class="obj-item" title="摧毁水晶"
                ><img class="obj-icon-img" :src="`/images/inhibitor-${team.teamId}.png`" />
                {{ team.inhibitorKills }}</span
              >
              <span class="obj-item" title="击杀纳什男爵"
                ><img class="obj-icon-img" :src="`/images/baron-${team.teamId}.png`" />
                {{ team.baronKills }}</span
              >
              <span class="obj-item" title="击杀巨龙"
                ><img class="obj-icon-img" :src="`/images/dragon-${team.teamId}.png`" />
                {{ team.dragonKills }}</span
              >
              <span class="obj-item" title="击杀峡谷先锋 / 虚空巢虫"
                ><img class="obj-icon-img" :src="`/images/herald-${team.teamId}.png`" />
                {{ team.riftHeraldKills }}</span
              >
            </div>

            <div class="team-header-spacer"></div>

            <div class="team-header-right">
              <span class="header-items">{{
                $t("search.items")
              }}</span>
              <span class="header-kda">{{ $t("career.kda") }}</span>
              <span class="header-cs">{{ $t("search.cs") }}</span>
              <span class="header-gold">{{ $t("search.gold") }}</span>
              <span class="header-damage">{{
                $t("search.damage")
              }}</span>
            </div>
          </div>

          <!-- 玩家列表 -->
          <div class="players-table">
            <div
              v-for="p in team.players"
              :key="p.participantId"
              :class="[
                'player-row',
                {
                  'highlight-row': p.puuid === myPuuid,
                  'win-row': team.win,
                  'lose-row': !team.win,
                },
              ]"
            >
              <!-- 头像及技能/符文 -->
              <div class="player-avatar-col">
                <div class="row-avatar-box">
                  <LcuImage
                    :src="p.championIconUrl"
                    class="row-avatar"
                    alt="champ"
                  />
                  <span class="row-level-overlay">{{ p.level }}</span>
                </div>
                <div class="row-spell-rune-row">
                  <div class="row-spell-col">
                    <LcuImage
                      :src="p.spell1Url"
                      class="row-spell"
                      alt="s1"
                    />
                    <LcuImage
                      :src="p.spell2Url"
                      class="row-spell"
                      alt="s2"
                    />
                  </div>
                  <div
                    v-if="queueId !== 2400 && queueId !== 2450"
                    class="row-rune"
                  >
                    <LcuImage
                      :src="p.runeUrl"
                      class="row-rune-img"
                      alt="rune"
                    />
                  </div>
                </div>
              </div>

              <!-- 名字（可点击搜索，机器人除外） -->
              <div class="player-name-col">
                <span
                  :class="[
                    'row-name',
                    {
                      'highlight-user': p.puuid === myPuuid,
                      'bot-player': !p.summonerId,
                    },
                  ]"
                  @click="
                    p.summonerId &&
                    emit('search-player', p.summonerId, p.name)
                  "
                  :title="p.summonerId ? `搜索 ${p.name}` : '机器人'"
                >
                  {{ p.name }}
                </span>
                <span
                  v-if="participantRanks[p.puuid]"
                  class="row-rank-badge"
                  :title="`段位: ${participantRanks[p.puuid]}`"
                >
                  {{ participantRanks[p.puuid] }}
                </span>
              </div>

              <div class="player-spacer"></div>

              <!-- 装备栏 -->
              <div class="player-items-col">
                <div class="player-items-wrap">
                  <div
                    v-if="(queueId === 2400 || queueId === 2450) && Boolean(p.augmentIconUrls?.length)"
                    class="row-augment-grid"
                  >
                    <n-tooltip
                      v-for="(url, idx) in p.augmentIconUrls"
                      :key="'aug-' + idx"
                      trigger="hover"
                      placement="top"
                    >
                      <template #trigger>
                        <div class="row-augment-slot">
                          <LcuImage :src="url" class="row-item-img" alt="aug" />
                        </div>
                      </template>
                      <div class="augment-tooltip">
                        <div class="augment-tooltip-name">{{ p.augmentNames?.[idx] || "海克斯强化" }}</div>
                      </div>
                    </n-tooltip>
                  </div>
                  <div class="row-items-row">
                    <div class="row-items-grid">
                      <div v-for="idx in 6" :key="idx" class="row-item-slot">
                        <LcuImage v-if="p.items[idx - 1]" :src="p.items[idx - 1]" class="row-item-img" alt="item" />
                      </div>
                    </div>
                    <div class="row-ward-slot">
                      <LcuImage v-if="p.ward" :src="p.ward" class="row-item-img" alt="ward" />
                    </div>
                  </div>
                </div>
              </div>

              <!-- KDA -->
              <div class="player-kda-col">
                <span class="row-kda-text">
                  {{ p.kills }}/<span class="death-red">{{
                    p.deaths
                  }}</span
                  >/{{ p.assists }}
                </span>
              </div>

              <!-- 补兵 -->
              <div class="player-cs-col">
                <span class="row-cs-text">{{ p.cs }}</span>
              </div>

              <!-- 金币 -->
              <div class="player-gold-col">
                <span class="row-gold-text">{{
                  p.gold.toLocaleString()
                }}</span>
              </div>

              <!-- 伤害 -->
              <div class="player-damage-col">
                <span class="row-damage-text">{{
                  p.damage.toLocaleString()
                }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="!details && !loading" class="detail-empty"></div>
  </div>
</template>

<style scoped>
/* 右侧详情面板 */
.right-detail-panel {
  background: transparent;
  border: none;
  box-shadow: none;
  min-height: 640px;
}

.detail-content {
  position: relative;
}

.detail-loading,
.detail-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 700px;
  color: var(--text-muted);
  font-size: 0.85rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.04);
  backdrop-filter: blur(15px);
  -webkit-backdrop-filter: blur(15px);
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 12px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 详情 Banner */
.detail-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  margin: 12px 12px 0 12px;
  border: 1px solid rgba(0, 0, 0, 0.05);
  border-radius: 8px;
  box-shadow: var(--shadow-sm);
}

.detail-banner.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.detail-banner.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.banner-main {
  display: flex;
  align-items: center;
  gap: 12px;
}

.banner-map-icon {
  width: 54px;
  height: 54px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
  flex-shrink: 0;
}

.banner-map-icon img {
  width: 100%;
  height: 100%;
  display: block;
}

.banner-left {
  display: flex;
  flex-direction: column;
}

.banner-result {
  font-size: 1.25rem;
  font-weight: 800;
  margin: 0 0 2px;
}

.banner-result.win {
  color: var(--win-color);
}
.banner-result.lose {
  color: var(--loss-color);
}

.banner-subtext {
  font-size: 0.75rem;
  color: var(--text-muted);
}

.copy-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.copy-btn:hover {
  background-color: var(--card-bg);
}

[data-theme="dark"] .copy-btn:hover {
  background-color: rgba(30, 41, 59, 0.9);
  border-color: var(--primary-color);
  color: var(--primary-color);
}

.copy-btn svg {
  width: 14px;
  height: 14px;
}

/* 队伍 block */
.teams-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
}

.team-block {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  background: var(--card-bg);
  box-shadow: var(--shadow-sm);
}

.team-block.win-block {
  border-color: var(--win-border);
}

.team-block.lose-block {
  border-color: var(--loss-border);
}

.team-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  font-size: 0.85rem;
}

.team-header-bar.win-bar {
  background-color: var(--win-bg);
  border-bottom: 1px solid var(--win-border);
}

.team-header-bar.lose-bar {
  background-color: var(--loss-bg);
  border-bottom: 1px solid var(--loss-border);
}

.team-result-label {
  font-weight: bold;
}
.win-text {
  color: var(--win-color);
}
.lose-text {
  color: var(--loss-color);
}

.team-objectives {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-muted);
  font-weight: 500;
  font-size: 0.8rem;
}

.obj-item {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.obj-icon {
  font-size: 0.85rem;
}

.obj-icon-img {
  width: 16px;
  height: 16px;
  object-fit: contain;
  vertical-align: middle;
}

/* 玩家列表 Table 行 */
.players-table {
  display: flex;
  flex-direction: column;
}

.player-row {
  display: flex;
  align-items: center;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border-color);
  font-size: 0.8rem;
  color: var(--text-color);
}

.player-row:last-child {
  border-bottom: none;
}

/* 玩家高亮行 */
.player-row.highlight-row.win-row {
  background-color: var(--win-bg) !important;
}

.player-row.highlight-row.lose-row {
  background-color: var(--loss-bg) !important;
}

.player-row.highlight-row.win-row .row-name,
.player-row.highlight-row.win-row .row-kda-text,
.player-row.highlight-row.win-row .row-kda-text .death-red,
.player-row.highlight-row.win-row .row-cs-text,
.player-row.highlight-row.win-row .row-gold-text,
.player-row.highlight-row.win-row .row-damage-text {
  color: var(--win-color) !important;
  font-weight: 800;
}

.player-row.highlight-row.lose-row .row-name,
.player-row.highlight-row.lose-row .row-kda-text,
.player-row.highlight-row.lose-row .row-kda-text .death-red,
.player-row.highlight-row.lose-row .row-cs-text,
.player-row.highlight-row.lose-row .row-gold-text,
.player-row.highlight-row.lose-row .row-damage-text {
  color: var(--loss-color) !important;
  font-weight: 800;
}

/* 1. 头像区 */
.player-avatar-col {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 120px;
  flex-shrink: 0;
}

.row-avatar-box {
  position: relative;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.row-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.row-level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 14px;
  height: 14px;
  line-height: 12px;
  background: var(--text-color);
  color: var(--bg-color);
  border-radius: 50%;
  font-size: 0.58rem;
  font-weight: bold;
  text-align: center;
  border: 1px solid var(--card-bg);
}

[data-theme="dark"] .row-level-overlay {
  background: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255, 255, 255, 0.2);
}

.row-spell-rune-row {
  display: flex;
  align-items: center;
  gap: 3px;
}

.row-spell-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row-spell {
  width: 18px;
  height: 18px;
  border-radius: 2px;
  border: 1px solid var(--border-color);
}

.row-rune {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

.row-rune-img {
  width: 20px;
  height: 20px;
  border-radius: 50%;
}

.player-name-col {
  max-width: 140px;
  min-width: 0;
  padding-right: 6px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 4px;
}

.row-rank-badge {
  display: inline-flex;
  align-items: center;
  font-size: 10px;
  line-height: 1.2;
  color: var(--primary-color);
  background: rgba(142, 68, 173, 0.08);
  padding: 1px 4px;
  border-radius: 4px;
  font-weight: 500;
  white-space: nowrap;
  flex-shrink: 0;
}

.row-name {
  display: block;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--text-color);
  cursor: pointer;
  transition: color 0.15s;
}

.row-name:hover {
  color: var(--primary-color);
}

.bot-player {
  cursor: default;
  color: var(--text-dimmed);
}

.bot-player:hover {
  color: var(--text-dimmed);
}

.highlight-user {
  color: #2ecc71 !important;
  font-weight: 800;
}

.highlight-user:hover {
  color: #27ae60 !important;
}

/* 3. 装备区 */
.player-items-col {
  display: flex;
  align-items: center;
  gap: 3px;
  min-width: 210px;
  flex-shrink: 0;
}

.player-items-wrap {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 3px;
}
.row-items-row {
  display: flex;
  align-items: center;
  gap: 3px;
}

/* 海克斯强化网格 */
.row-augment-grid {
  display: flex;
  gap: 2px;
  margin-right: 4px;
}
.row-augment-slot {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(168, 85, 247, 0.45);
  background-color: rgba(147, 51, 234, 0.12);
  transition: all 0.2s ease;
  cursor: pointer;
}
.row-augment-slot:hover {
  border-color: #c084fc;
  box-shadow: 0 0 8px rgba(192, 132, 252, 0.6);
  transform: translateY(-1px) scale(1.05);
}

.augment-tooltip {
  max-width: 280px;
  padding: 8px 10px;
  text-align: left;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
}

.augment-tooltip-name {
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--text-color);
  letter-spacing: 0.2px;
}

.row-items-grid {
  display: flex;
  gap: 1px;
}

.row-item-slot {
  width: 28px;
  height: 28px;
  background: rgba(0, 0, 0, 0.03);
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.row-item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.row-ward-slot {
  width: 28px;
  height: 28px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid var(--accent-color, #e6a23c);
  background-color: rgba(230, 162, 60, 0.03);
}

/* 4. KDA */
.player-kda-col {
  width: 70px;
  text-align: center;
  font-weight: 600;
  flex-shrink: 0;
}

.row-kda-text {
  font-size: 0.8rem;
}

/* 5. 补兵 */
.player-cs-col {
  width: 42px;
  text-align: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.row-cs-text {
  font-size: 0.8rem;
}

/* 6. 金币 */
.player-gold-col {
  width: 55px;
  text-align: right;
  color: var(--text-muted);
  flex-shrink: 0;
}

.row-gold-text {
  font-size: 0.8rem;
}

/* 7. 伤害 */
.player-damage-col {
  width: 60px;
  text-align: right;
  font-weight: 700;
  color: var(--text-color);
  flex-shrink: 0;
}

.row-damage-text {
  font-size: 0.8rem;
}

.player-spacer,
.team-header-spacer {
  flex: 1;
}

.team-header-right {
  display: flex;
  align-items: center;
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
  padding-right: 2px;
}

.header-items {
  min-width: 190px;
  text-align: center;
}

.header-kda {
  width: 70px;
  text-align: center;
}

.header-cs {
  width: 42px;
  text-align: center;
}

.header-gold {
  width: 55px;
  text-align: right;
}

.header-damage {
  width: 60px;
  text-align: right;
}
</style>
