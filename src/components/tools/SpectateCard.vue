<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { lcuRequest, cleanError } from "../../api/lcu";
import { useToast } from "../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NInput, NButton, NSelect, NCollapse, NCollapseItem } from "naive-ui";

const { showToast } = useToast();
const { t } = useI18n();

const loading = ref(false);
const spectateSummonerName = ref("");
const spectateMethod = ref<"LCU" | "CMD">("LCU");

// 观战启动
async function handleSpectate() {
  if (!spectateSummonerName.value.trim()) return;
  loading.value = true;
  try {
    const name = spectateSummonerName.value.trim().replace(/[⁦⁩]/g, "");

    if (spectateMethod.value === "CMD") {
      // CMD 方式：通过 SGP 获取凭据后直接启动 League of Legends.exe
      await invoke<string>("spectate_directly", {
        params: { summoner_name: name },
      });
      showToast(t("tools.spectate.startedCmd"));
    } else {
      // LCU API 方式：通过 LCU 接口进行好友/对局观战
      const summonerResp = await lcuRequest<any>(
        "GET",
        `/lol-summoner/v1/summoners?name=${encodeURIComponent(name)}`,
      );
      if (!summonerResp.success || !summonerResp.data) {
        showToast(t("tools.spectate.notFound"), "error");
        return;
      }
      const puuid = summonerResp.data.puuid;

      // 1. 安全地从全量好友列表中匹配目标 puuid 提取对局 ID
      let gameIdFromFriend = "";
      const friendsResp = await lcuRequest<any[]>(
        "GET",
        "/lol-chat/v1/friends",
      );
      if (friendsResp.success && Array.isArray(friendsResp.data)) {
        const friend = friendsResp.data.find(
          (f: any) => f.puuid === puuid || f.id === puuid,
        );
        const rawGameId = friend?.lol?.gameId;
        if (
          rawGameId &&
          String(rawGameId) !== "0" &&
          String(rawGameId) !== "null"
        ) {
          gameIdFromFriend = String(rawGameId);
          console.log("从好友列表匹配到当前对局 ID:", gameIdFromFriend);
        }
      }

      // 2. 从 Lobby 大厅中匹配当前自定义房间的对局 ID
      let gameIdFromLobby = "";
      const lobbyResp = await lcuRequest<any>("GET", "/lol-lobby/v2/lobby");
      if (lobbyResp.success && lobbyResp.data) {
        const config = lobbyResp.data.gameConfig;
        if (
          config &&
          config.id &&
          String(config.id) !== "0" &&
          String(config.id) !== "null"
        ) {
          gameIdFromLobby = String(config.id);
          console.log("从 Lobby 大厅匹配到自定义对局 ID:", gameIdFromLobby);
        }
      }

      // 3. 自定义对局必须传对局 ID 才能加载观战，否则退化为名字匹配
      const targetGameId = gameIdFromFriend || gameIdFromLobby || name;

      // 4. 发送 LCU 观战启动请求
      const resp = await lcuRequest<any>(
        "POST",
        "/lol-spectator/v1/spectate/launch",
        {
          allowObserveMode: "ALL",
          dropInSpectateGameId: targetGameId,
          gameQueueType: "",
          puuid: puuid,
        },
      );
      if (resp.success) {
        showToast(t("tools.spectate.success"), "success");
      } else {
        // 5. 观战降级兜底：LCU 方式发生任何失败，立刻尝试通过 CMD 方式
        console.warn("LCU 观战失败，尝试通过 CMD 方式兜底拉起...", {
          targetGameId,
          puuid,
          error: resp.error,
        });

        try {
          await invoke<string>("spectate_directly", {
            params: { summoner_name: name },
          });
          showToast(t("tools.spectate.fallbackCmd"), "success");
        } catch (cmdErr: any) {
          console.error("CMD 兜底观战亦告失败:", cmdErr);
          showToast(
            t("tools.spectate.failed", {
              error:
                cleanError(resp.error || "该召唤师当前可能无法被观战") +
                " (自定义/新开局请先在官方客户端右键尝试观战以同步密钥)",
            }),
            "error",
          );
        }
      }
    }
  } catch (e: any) {
    showToast(t("tools.spectate.error", { error: cleanError(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="spectate">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left">
            <div class="icon-container">
              <svg
                class="header-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path
                  d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"
                ></path>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.spectate.title") }}</h3>
              <span class="card-desc">{{ t("tools.spectate.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{ t("tools.spectate.expand") }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.spectate.nameLabel") }}</span>
        <n-input
          v-model:value="spectateSummonerName"
          :placeholder="t('tools.spectate.namePlaceholder')"
          clearable
          style="max-width: 300px"
        >
          <template #suffix>
            <n-button
              size="small"
              type="primary"
              :disabled="!spectateSummonerName.trim()"
              @click="handleSpectate"
            >
              {{ t("tools.spectate.btn") }}
            </n-button>
          </template>
        </n-input>
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.spectate.methodLabel") }}</span>
        <n-select
          v-model:value="spectateMethod"
          :options="[
            { label: 'LCU API', value: 'LCU' },
            { label: 'CMD', value: 'CMD' },
          ]"
          style="width: 120px"
          size="small"
        />
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped>
.collapse-header-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.collapse-left {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
  stroke-width: 2px;
}

.title-container {
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 0.88rem;
  font-weight: bold;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.collapse-right-status {
  margin-left: auto;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-right: 10px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px dashed var(--border-color);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}
</style>
