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
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border-color); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--primary-color); }
</style>
