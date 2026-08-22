<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { updateConfig, WEGAME_MARKER } from "../../api/lcu";
import { useSettingsAutoSave } from "../../composables/useSettingsAutoSave";
import { useToast } from "../../composables/useToast";
import { useI18n } from "vue-i18n";

const { config } = useSettingsAutoSave();
const { showToast } = useToast();
const { t } = useI18n();

// 自动检测客户端路径（追加到列表）
async function handleDetectPath() {
  try {
    const path = await invoke<string | null>("detect_lol_path");
    if (path) {
      if (!config.value) return;
      const paths = config.value.General.LolPath || [];
      if (!paths.includes(path)) {
        paths.push(path);
        config.value.General.LolPath = paths;
        await updateConfig(config.value);
        showToast("已添加: " + path);
      } else {
        showToast("该路径已存在");
      }
    } else {
      showToast("未检测到运行中的英雄联盟客户端", "error");
    }
  } catch (e: unknown) {
    showToast("检测失败: " + String(e), "error");
  }
}

// 手动选择客户端目录（追加到列表）
async function handleBrowseFolder() {
  try {
    const path = await invoke<string | null>("select_lol_folder");
    if (path) {
      if (!config.value) return;
      const paths = config.value.General.LolPath || [];
      if (!paths.includes(path)) {
        paths.push(path);
        config.value.General.LolPath = paths;
        await updateConfig(config.value);
        showToast("已添加: " + path);
      } else {
        showToast("该路径已存在");
      }
    }
  } catch (e: unknown) {
    showToast("选择失败: " + String(e), "error");
  }
}

// 添加 WeGame 启动项（自动检测安装位置，检测不到则提示）
async function handleAddWeGame() {
  try {
    if (!config.value) return;
    const paths = config.value.General.LolPath || [];
    if (paths.includes(WEGAME_MARKER)) {
      showToast("WeGame 启动项已存在");
      return;
    }
    const path = await invoke<string | null>("detect_wegame_path");
    if (path) {
      paths.push(WEGAME_MARKER);
      config.value.General.WegamePath = path;
      config.value.General.LolPath = paths;
      await updateConfig(config.value);
      showToast("已添加 WeGame 启动项 (" + path + ")");
    } else {
      showToast("未检测到 WeGame 安装路径，请先安装 WeGame", "error");
    }
  } catch (e: unknown) {
    showToast("检测失败: " + String(e), "error");
  }
}

// 手动设置 WeGame 路径（条目内点击编辑后保存）
async function handleEditWegamePath(val: string) {
  if (!config.value) return;
  const v = val.trim();
  config.value.General.WegamePath = v || null;
  await updateConfig(config.value);
}

// 删除指定路径
async function handleRemovePath(index: number) {
  if (!config.value) return;
  const removed = config.value.General.LolPath[index];
  config.value.General.LolPath.splice(index, 1);
  // 删除 WeGame 启动项时同步清空其路径，避免残留
  if (removed === WEGAME_MARKER) {
    config.value.General.WegamePath = null;
  }
  await updateConfig(config.value);
}

// 修改指定路径
async function handleEditPathDirect(index: number, val: string) {
  if (!config.value) return;
  const pathVal = val.trim();
  if (!pathVal) return;
  config.value.General.LolPath[index] = pathVal;
  await updateConfig(config.value);
}
</script>

<template>
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="lolpath">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{
              $t("settings.lolPathGroup")
            }}</span>
            <span class="card-desc">{{ $t("settings.lolPathDesc") }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              config?.General?.LolPath?.length
                ? $t("settings.pathSetCount", {
                    count: config.General.LolPath.length,
                  })
                : $t("settings.pathNotSet")
            }}</span>
          </div>
        </div>
      </template>
      <!-- 已保存的路径列表 -->
      <div
        v-for="(path, index) in config?.General?.LolPath || []"
        :key="index"
        class="path-item"
      >
        <template v-if="path === WEGAME_MARKER">
          <div class="wegame-item">
            <span class="wegame-badge">WeGame</span>
            <n-input
              class="path-input"
              :value="config?.General?.WegamePath ?? ''"
              @change="(val) => handleEditWegamePath(String(val))"
              :placeholder="t('settings.wegamePathPlaceholder')"
              style="flex: 1; min-width: 0"
            />
          </div>
          <n-button
            size="tiny"
            circle
            @click="handleRemovePath(index)"
            :title="t('settings.pathRemove')"
            >✕</n-button
          >
        </template>
        <template v-else>
          <n-input
            class="path-input"
            :value="path"
            @change="(val) => handleEditPathDirect(index, val)"
            :placeholder="t('settings.lolPathPlaceholder')"
            style="flex: 1; margin-right: 8px"
          />
          <n-button
            size="tiny"
            circle
            @click="handleRemovePath(index)"
            :title="t('settings.pathRemove')"
            >✕</n-button
          >
        </template>
      </div>
      <div v-if="!config?.General?.LolPath?.length" class="path-empty">
        {{ $t("settings.pathEmpty") }}
      </div>
      <!-- 操作按钮 -->
      <div class="path-actions">
        <n-button size="small" @click="handleDetectPath">{{
          $t("settings.detectBtn")
        }}</n-button>
        <n-button size="small" @click="handleBrowseFolder">{{
          $t("settings.browseBtn")
        }}</n-button>
        <n-button size="small" @click="handleAddWeGame">{{
          $t("settings.wegameAddBtn")
        }}</n-button>
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped src="./shared.css"></style>

<style scoped>
/* 客户端路径列表 */
.path-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  margin-bottom: 6px;
  background: var(--card-bg);
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
}
.path-item:hover {
  border-color: rgba(0, 159, 170, 0.3);
  box-shadow: 0 0 0 1px rgba(0, 159, 170, 0.15);
}
.path-input {
  font-size: 0.82rem;
  color: var(--text-color);
  flex: 1;
  margin-right: 8px;
  border: 1px solid transparent;
  background: transparent;
  padding: 4px 8px;
  border-radius: 4px;
  outline: none;
  transition: all 0.2s;
}
.path-input:focus {
  border-color: var(--primary-color);
  background: var(--card-bg);
}
.path-remove-btn {
  background: transparent;
  border: none;
  color: var(--text-dimmed);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 2px 6px;
  border-radius: 4px;
  flex-shrink: 0;
  transition: all 0.15s;
}
.path-remove-btn:hover {
  color: var(--loss-color);
  background: var(--loss-bg);
}
.path-empty {
  font-size: 0.8rem;
  color: var(--text-dimmed);
  text-align: center;
  padding: 12px 0;
}
/* WeGame 启动项条目 */
.wegame-item {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.wegame-badge {
  flex-shrink: 0;
  padding: 2px 10px;
  border-radius: 4px;
  font-size: 0.82rem;
  font-weight: 600;
  color: #fff;
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--primary-color) 80%, #ffffff),
    var(--primary-color)
  );
}
.path-actions {
  display: flex;
  gap: 8px;
  margin-top: 8px;
  justify-content: flex-end;
  flex-wrap: wrap;
}
</style>
