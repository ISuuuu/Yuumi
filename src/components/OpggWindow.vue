<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import OpggModal from "./OpggModal.vue";

function handleClose() {
  getCurrentWindow().close();
}
</script>

<template>
  <OpggModal @close="handleClose" />
</template>

<style>
:root {
  --primary-color: #009faa;
  --primary-color-hover: #008a94;
  --primary-color-alpha-15: rgba(0, 159, 170, 0.15);
  --primary-color-alpha-30: rgba(0, 159, 170, 0.3);
  --primary-color-alpha-40: rgba(0, 159, 170, 0.4);

  --bg-color-gradient: linear-gradient(135deg, #ffffff 0%, #fcfdfe 40%, #f3f6fc 100%);
  --bg-color: #f7fafc;
  --sidebar-bg: rgba(255, 255, 255, 0.85);
  --card-bg: rgba(255, 255, 255, 0.82);
  --card-bg-hover: rgba(255, 255, 255, 0.96);
  --border-color: rgba(0, 0, 0, 0.04);
  --border-color-hover: rgba(0, 0, 0, 0.08);
  --hover-bg: rgba(0, 0, 0, 0.04);
  --hover-bg-strong: rgba(0, 0, 0, 0.06);
  
  --text-color: #0f172a;
  --text-muted: #334155;
  --text-dimmed: #64748b;

  --win-color: #10b981;
  --win-bg: rgba(16, 185, 129, 0.08);
  --win-border: rgba(16, 185, 129, 0.18);
  --win-glow: rgba(16, 185, 129, 0.08);

  --loss-color: #ef4444;
  --loss-bg: rgba(239, 68, 68, 0.07);
  --loss-border: rgba(239, 68, 68, 0.18);
  --loss-glow: rgba(239, 68, 68, 0.08);

  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Roboto, sans-serif;
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --shadow-sm: 0 2px 12px rgba(0, 0, 0, 0.015);
  --shadow-md: 0 8px 30px rgba(0, 0, 0, 0.03);
  --shadow-lg: 0 16px 40px rgba(0, 0, 0, 0.06);
  --glass-filter: blur(16px);
}

[data-theme="dark"] {
  /* 暗黑主题变量 */
  --bg-color-gradient: linear-gradient(135deg, #0f172a 0%, #111827 40%, #1e293b 100%);
  --bg-color: #0f172a;
  --sidebar-bg: rgba(17, 24, 39, 0.9);
  --card-bg: rgba(30, 41, 59, 0.85);
  --card-bg-hover: rgba(30, 41, 59, 0.95);
  --border-color: rgba(255, 255, 255, 0.06);
  --border-color-hover: rgba(255, 255, 255, 0.12);
  --hover-bg: rgba(255, 255, 255, 0.06);
  --hover-bg-strong: rgba(255, 255, 255, 0.1);
  
  --text-color: #f1f5f9;
  --text-muted: #94a3b8;
  --text-dimmed: #64748b;

  --win-color: #34d399;
  --win-bg: rgba(16, 185, 129, 0.12);
  --win-border: rgba(16, 185, 129, 0.25);
  --win-glow: rgba(16, 185, 129, 0.12);

  --loss-color: #f87171;
  --loss-bg: rgba(239, 68, 68, 0.1);
  --loss-border: rgba(239, 68, 68, 0.25);
  --loss-glow: rgba(239, 68, 68, 0.1);
  --death-color: #f87171;

  --shadow-sm: 0 2px 12px rgba(0, 0, 0, 0.2);
  --shadow-md: 0 8px 30px rgba(0, 0, 0, 0.3);
  --shadow-lg: 0 16px 40px rgba(0, 0, 0, 0.4);
  --glass-filter: blur(16px);
}

* { box-sizing: border-box; }

body {
  margin: 0;
  font-family: var(--font-sans);
  background-color: var(--bg-color);
  color: var(--text-color);
  overflow: hidden;
  user-select: none;
}

::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.06); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--primary-color); }
</style>
