<template>
  <Modal :open="open" title="安装 acme.sh" :footer="null" :width="640" @cancel="close">
    <template v-if="install">
      <Alert
        type="warning"
        show-icon
        :message="`未检测到 acme.sh（服务运行环境：${install.os_name}），请按以下步骤在服务器上安装`"
        style="margin-bottom: 18px"
      />

      <div v-for="(s, i) in install.steps" :key="i" class="install-step">
        <div class="step-title">{{ s.title }}</div>
        <div v-for="cmd in s.commands" :key="cmd" class="cmd-block">
          <span class="cmd-prompt">$</span>
          <code class="cmd-text">{{ cmd }}</code>
          <Button class="copy-btn" size="small" type="text" @click="copy(cmd)">
            <span class="btn-inner">
              <component :is="copied === cmd ? Check : Copy" :size="13" />
              {{ copied === cmd ? "已复制" : "复制" }}
            </span>
          </Button>
        </div>
      </div>

      <ul v-if="install.notes.length" class="notes">
        <li v-for="(n, i) in install.notes" :key="i">{{ n }}</li>
      </ul>

      <div class="foot">
        <Button type="primary" ghost @click="redetect">
          <span class="btn-inner"><RefreshCw :size="14" /> 重新检测</span>
        </Button>
      </div>
    </template>
  </Modal>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { Alert, Button, Modal, message } from "antdv-next";
import { Check, Copy, RefreshCw } from "@lucide/vue";
import type { AcmeInstallHint } from "../api/types";

const props = defineProps<{
  open: boolean;
  install: AcmeInstallHint | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
  (e: "redetect"): void;
}>();

const copied = ref("");

const copy = async (text: string) => {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = text;
    setTimeout(() => {
      if (copied.value === text) copied.value = "";
    }, 2000);
  } catch {
    message.error("复制失败，请手动选择命令复制");
  }
};

const close = () => emit("update:open", false);

const redetect = () => {
  emit("redetect");
  close();
};
</script>

<style lang="scss" scoped>
.install-step {
  margin-bottom: 18px;

  .step-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ant-color-text, #374151);
    margin-bottom: 8px;
  }
}

.cmd-block {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  background: #1e2230;
  border-radius: 8px;
  padding: 11px 12px;
  margin-bottom: 8px;

  .cmd-prompt {
    flex: none;
    color: #4f8cff;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 13px;
    user-select: none;
  }

  .cmd-text {
    flex: 1;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 13px;
    color: #e6e9f2;
    word-break: break-all;
    background: none;
  }

  .copy-btn {
    flex: none;
    color: #9aa4bf;
  }
}

.notes {
  margin: 4px 0 0;
  padding-left: 18px;
  font-size: 12.5px;
  color: var(--ant-color-text-secondary, #6b7280);
  line-height: 1.9;
}

.foot {
  margin-top: 18px;
  display: flex;
  justify-content: flex-end;
}

.btn-inner {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
</style>
