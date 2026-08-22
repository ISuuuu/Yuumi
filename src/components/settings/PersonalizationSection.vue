<script setup lang="ts">
import { inject } from "vue";
import {
  updateThemeColor,
  updateDeathColor,
  updateCardColors,
} from "../../utils/theme";
import ColorPickerWithAlpha from "../ColorPickerWithAlpha.vue";
import { setLocale } from "../../i18n";
import { useSettingsAutoSave } from "../../composables/useSettingsAutoSave";
import { useToast } from "../../composables/useToast";
import { useI18n } from "vue-i18n";

const { config, autoSave } = useSettingsAutoSave();
const applyMicaEffect = inject<(enabled: boolean) => void>("applyMicaEffect");
const { showToast } = useToast();
const { t } = useI18n();

function toColor6(color: string | undefined): string {
  if (!color) return "#000000";
  if (color.startsWith("#") && color.length === 9) {
    return "#" + color.slice(3);
  }
  return color;
}

function toColor8(color: string): string {
  if (color.startsWith("#") && color.length === 7) {
    return "#ff" + color.slice(1);
  }
  return color;
}

function onCardColorChange(
  val: string,
  field: "WinCardColor" | "LoseCardColor" | "RemakeCardColor",
) {
  if (!config.value?.Personalization) return;
  config.value.Personalization[field] = val;

  // 实时更新全局 CSS 变量
  updateCardColors(
    config.value.Personalization.WinCardColor,
    config.value.Personalization.LoseCardColor,
    config.value.Personalization.RemakeCardColor,
  );
}

function onThemeColorSelect(color: string) {
  if (config.value?.Personalization) {
    config.value.Personalization.ThemeColor = color;
  }
  updateThemeColor(color);
  autoSave();
}

function onDeathColorSelect(
  color: string,
  field: "LightDeathsNumberColor" | "DarkDeathsNumberColor",
) {
  const color8 = toColor8(color);
  if (config.value?.Personalization) {
    config.value.Personalization[field] = color8;
  }
  // 实时更新 CSS 变量
  const light =
    field === "LightDeathsNumberColor"
      ? color
      : toColor6(config.value?.Personalization?.LightDeathsNumberColor);
  const dark =
    field === "DarkDeathsNumberColor"
      ? color
      : toColor6(config.value?.Personalization?.DarkDeathsNumberColor);
  updateDeathColor(light, dark);
  autoSave();
}

const DEFAULT_COLORS = {
  ThemeColor: "#00d2c4",
  WinCardColor: "#3339b01b",
  LoseCardColor: "#33d3190c",
  RemakeCardColor: "#33a2a2a2",
  LightDeathsNumberColor: "#ffb60000",
  DarkDeathsNumberColor: "#ffff8d8d",
};

function resetThemeColor() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.ThemeColor = DEFAULT_COLORS.ThemeColor;
  updateThemeColor(DEFAULT_COLORS.ThemeColor);
  autoSave();
}

function resetCardColors() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.WinCardColor = DEFAULT_COLORS.WinCardColor;
  config.value.Personalization.LoseCardColor = DEFAULT_COLORS.LoseCardColor;
  config.value.Personalization.RemakeCardColor = DEFAULT_COLORS.RemakeCardColor;
  updateCardColors(
    DEFAULT_COLORS.WinCardColor,
    DEFAULT_COLORS.LoseCardColor,
    DEFAULT_COLORS.RemakeCardColor,
  );
  autoSave();
}

function resetDeathColors() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.LightDeathsNumberColor =
    DEFAULT_COLORS.LightDeathsNumberColor;
  config.value.Personalization.DarkDeathsNumberColor =
    DEFAULT_COLORS.DarkDeathsNumberColor;
  updateDeathColor(
    toColor6(DEFAULT_COLORS.LightDeathsNumberColor),
    toColor6(DEFAULT_COLORS.DarkDeathsNumberColor),
  );
  autoSave();
}

function applyThemeMode(mode: string) {
  const root = document.documentElement;
  if (mode === "Auto") {
    root.removeAttribute("data-theme");
    localStorage.setItem("yuumi_theme", "Auto");
  } else if (mode === "Light" || mode === "Dark") {
    root.setAttribute("data-theme", mode.toLowerCase());
    localStorage.setItem("yuumi_theme", mode);
  }
}
</script>

<template>
  <!-- 5. 个性化 -->
  <template v-if="config">
    <div class="group-header">{{ $t("settings.personalizationGroup") }}</div>

  <div class="card-item border-bottom">
    <div class="card-left">
      <h3 class="card-title">{{ $t("settings.micaTitle") }}</h3>
      <span class="card-desc">{{ $t("settings.micaDesc") }}</span>
    </div>
    <div class="card-right">
      <n-switch
        v-model:value="config.Personalization.MicaEnabled"
        @update:value="
          autoSave();
          applyMicaEffect?.(config.Personalization.MicaEnabled);
        "
      />
    </div>
  </div>

  <!-- 应用主题 -->
  <div class="card-item border-bottom">
    <div class="card-left">
      <h3 class="card-title">{{ $t("settings.themeModeTitle") }}</h3>
      <span class="card-desc">{{ $t("settings.themeModeDesc") }}</span>
    </div>
    <div class="card-right">
      <n-select
        v-model:value="config.Personalization.ThemeMode"
        :options="[
          { label: $t('settings.themeModeLight'), value: 'Light' },
          { label: $t('settings.themeModeDark'), value: 'Dark' },
          { label: $t('settings.themeModeAuto'), value: 'Auto' },
        ]"
        @update:value="
          (val) => {
            applyThemeMode(val);
            autoSave();
          }
        "
        style="width: 140px"
        size="small"
      />
    </div>
  </div>

  <!-- 主题色 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="themecolor">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{
              $t("settings.themeColorTitle")
            }}</span>
            <span class="card-desc">{{
              $t("settings.themeColorDesc")
            }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview"
              >#{{
                toColor6(config.Personalization.ThemeColor)?.replace(
                  "#",
                  "",
                )
              }}</span
            >
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{
          $t("settings.themeColorPickerLabel")
        }}</span>
        <div style="width: 100px; flex-shrink: 0">
          <n-color-picker
            :value="toColor6(config.Personalization.ThemeColor)"
            :show-alpha="false"
            @update:value="onThemeColorSelect"
            size="small"
          />
        </div>
      </div>
      <div class="reset-row">
        <n-button size="small" @click="resetThemeColor">{{
          $t("settings.resetColors")
        }}</n-button>
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 对局卡片颜色 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="cardcolors">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{
              $t("settings.cardColorTitle")
            }}</span>
            <span class="card-desc">{{
              $t("settings.cardColorDesc")
            }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              $t("settings.cardColorStatusSet")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ $t("settings.winCard") }}</span>
        <ColorPickerWithAlpha
          :value="
            config ? config.Personalization.WinCardColor : '#ffffffff'
          "
          @update:value="(val) => onCardColorChange(val, 'WinCardColor')"
          @save="autoSave"
          size="small"
          style="width: 120px; flex-shrink: 0"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ $t("settings.loseCard") }}</span>
        <ColorPickerWithAlpha
          :value="
            config ? config.Personalization.LoseCardColor : '#ffffffff'
          "
          @update:value="(val) => onCardColorChange(val, 'LoseCardColor')"
          @save="autoSave"
          size="small"
          style="width: 120px; flex-shrink: 0"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ $t("settings.remakeCard") }}</span>
        <ColorPickerWithAlpha
          :value="
            config ? config.Personalization.RemakeCardColor : '#ffffffff'
          "
          @update:value="(val) => onCardColorChange(val, 'RemakeCardColor')"
          @save="autoSave"
          size="small"
          style="width: 120px; flex-shrink: 0"
        />
      </div>
      <div class="reset-row">
        <n-button size="small" @click="resetCardColors">{{
          $t("settings.resetColors")
        }}</n-button>
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 死亡数字体颜色 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="deathfontcolor">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{
              $t("settings.deathColorTitle")
            }}</span>
            <span class="card-desc">{{
              $t("settings.deathColorDesc")
            }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              $t("settings.cardColorStatusSet")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{
          $t("settings.lightDeathLabel")
        }}</span>
        <div style="width: 100px; flex-shrink: 0">
          <n-color-picker
            :value="
              config
                ? toColor6(config.Personalization.LightDeathsNumberColor)
                : ''
            "
            :show-alpha="false"
            @update:value="
              (val) => onDeathColorSelect(val, 'LightDeathsNumberColor')
            "
            size="small"
          />
        </div>
      </div>
      <div class="setting-row">
        <span class="setting-label">{{
          $t("settings.darkDeathLabel")
        }}</span>
        <div style="width: 100px; flex-shrink: 0">
          <n-color-picker
            :value="
              config
                ? toColor6(config.Personalization.DarkDeathsNumberColor)
                : ''
            "
            :show-alpha="false"
            @update:value="
              (val) => onDeathColorSelect(val, 'DarkDeathsNumberColor')
            "
            size="small"
          />
        </div>
      </div>
      <div class="reset-row">
        <n-button size="small" @click="resetDeathColors">{{
          $t("settings.resetColors")
        }}</n-button>
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 界面缩放 -->
  <div class="card-item border-bottom">
    <div class="card-left">
      <h3 class="card-title">{{ $t("settings.dpiScaleTitle") }}</h3>
      <span class="card-desc">{{ $t("settings.dpiScaleDesc") }}</span>
    </div>
    <div class="card-right">
      <n-select
        v-model:value="config.Personalization.DpiScale"
        :options="[
          { label: $t('settings.dpiScaleAuto'), value: 'Auto' },
          { label: '100%', value: '100' },
          { label: '125%', value: '125' },
          { label: '150%', value: '150' },
        ]"
        @update:value="
          () => {
            autoSave({ silent: true });
            showToast(t('settings.dpiScaleSaved'));
          }
        "
        style="width: 140px"
        size="small"
      />
    </div>
  </div>

  <!-- 语言 -->
  <div class="card-item border-bottom">
    <div class="card-left">
      <h3 class="card-title">{{ $t("settings.langTitle") }}</h3>
      <span class="card-desc">{{ $t("settings.langDesc") }}</span>
    </div>
    <div class="card-right">
      <n-select
        v-model:value="config.Personalization.Language"
        :options="[
          { label: $t('settings.langAuto'), value: 'Auto' },
          { label: '简体中文', value: 'zh_CN' },
          { label: '繁體中文', value: 'zh_TW' },
          { label: 'English', value: 'en_US' },
        ]"
        @update:value="
          (val: string) => {
            autoSave({ silent: true });
            setLocale(val);
            showToast(t('settings.langSaved'));
          }
        "
        style="width: 140px"
        size="small"
      />
    </div>
  </div>
  </template>
</template>

<style scoped src="./shared.css"></style>
