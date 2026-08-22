<script setup lang="ts">
import { useDialog, useMessage } from "naive-ui";
import { captureDialogApi, captureMessageApi } from "../composables/useToast";

// 在 Provider 子树内捕获 Message/Dialog 实例，供 Provider 之外的调用方（App.vue 等）使用
captureMessageApi(useMessage());
captureDialogApi(useDialog());
</script>

<template>
  <div style="display: none"></div>
</template>
