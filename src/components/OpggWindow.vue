<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { onMounted } from "vue";
import OpggModal from "./OpggModal.vue";
import NaiveUIBridge from "./NaiveUIBridge.vue";

function handleClose() {
  getCurrentWindow().close();
}

// 在组件挂载前立即同步主题，避免白屏闪烁
const savedTheme = localStorage.getItem("yuumi_theme");
const isSystemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
const isDark =
  savedTheme === "Dark" || (savedTheme !== "Light" && isSystemDark);
document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");

onMounted(() => {
  // 禁用右键菜单
  document.addEventListener("contextmenu", (e) => e.preventDefault());

  // 禁用刷新快捷键：F5 / Ctrl+R / Ctrl+Shift+R
  document.addEventListener("keydown", (e) => {
    if (
      e.key === "F5" ||
      (e.ctrlKey && e.key === "r") ||
      (e.ctrlKey && e.shiftKey && e.key === "R")
    ) {
      e.preventDefault();
    }
  });
});
</script>

<template>
  <n-config-provider>
    <n-message-provider>
      <n-dialog-provider>
        <NaiveUIBridge />
        <OpggModal @close="handleClose" />
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
html,
body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  background: var(--bg-color);
  color: var(--text-color);
  transition: background 0.25s, color 0.25s;
}

::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border-color); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--primary-color); }
</style>
