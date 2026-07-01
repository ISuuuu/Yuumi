<script setup lang="ts">
import { useMessage, useDialog } from "naive-ui";

const message = useMessage();
const dialog = useDialog();

// Expose Naive UI APIs globally for App.vue and legacy usage
(window as any).$message = message;
(window as any).$dialog = dialog;
</script>

<template>
  <div style="display: none"></div>
</template>
