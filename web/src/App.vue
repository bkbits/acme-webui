<template>
  <ConfigProvider :locale="zhCN" :theme="themeConfig">
    <RouterView />
  </ConfigProvider>
</template>

<script setup lang="ts">
import { ConfigProvider } from "antdv-next";
import zhCN from "antdv-next/locale/zh_CN";
import dayjs from "dayjs";
import "dayjs/locale/zh";
import { onMounted } from "vue";
import { useAuth } from "./composables/auth";
import { useTheme } from "./composables/theme";

dayjs.locale("zh");

const auth = useAuth();
const { themeConfig } = useTheme();
onMounted(() => {
  if (auth.token.value) {
    auth.ensureMe();
  }
});
</script>
