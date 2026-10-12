<template>
  <Modal
    :open="open"
    :title="cert ? '编辑证书配置' : '添加证书'"
    :width="660"
    :confirm-loading="saving"
    ok-text="保存"
    @ok="onOk"
    @cancel="close"
  >
    <Form ref="formRef" :model="form" :rules="rules" layout="vertical" style="margin-top: 8px">
      <FormItem label="名称（备注）" name="name">
        <Input
          v-model:value="form.name"
          placeholder="可选，便于识别，如: 官网证书"
          :maxlength="64"
        />
      </FormItem>

      <FormItem label="DNS 运营商配置" name="provider_id">
        <Select
          v-model:value="form.provider_id"
          placeholder="选择已配置的运营商凭据"
          :options="providerOptions"
        />
        <template v-if="providers.length === 0" #extra>
          <span style="color: var(--ant-color-warning, #fa8c16)"
            >尚未配置运营商，请先前往「运营商配置」页面添加</span
          >
        </template>
      </FormItem>

      <FormItem name="domains">
        <template #label>
          域名
          <span class="sub-text">（回车或逗号分隔添加，支持多域名与通配符 *.example.com）</span>
        </template>
        <Select
          v-model:value="form.domains"
          mode="tags"
          :token-separators="[',', ' ', '\n']"
          placeholder="例如: test.example.com, *.example.com"
        />
      </FormItem>

      <div class="form-grid">
        <FormItem label="公钥位置（证书文件）" name="cert_path">
          <Input v-model:value="form.cert_path" placeholder="/root/certs/public.pem" />
        </FormItem>
        <FormItem label="私钥位置（密钥文件）" name="key_path">
          <Input v-model:value="form.key_path" placeholder="/root/certs/private.key" />
        </FormItem>
      </div>

      <div class="form-grid">
        <FormItem label="CA 机构" name="ca">
          <Select v-model:value="form.ca" :options="caOptions" />
        </FormItem>
        <FormItem name="renewal_days">
          <template #label>
            续期周期（天）
            <span class="sub-text">（距上次颁发或添加 N 天后自动续期）</span>
          </template>
          <InputNumber v-model:value="form.renewal_days" :min="1" :max="3650" style="width: 100%" />
        </FormItem>
      </div>

      <FormItem name="post_command">
        <template #label>
          完成后命令
          <span class="sub-text">（可选，证书安装完成后执行，如 nginx -s reload）</span>
        </template>
        <TextArea v-model:value="form.post_command" :rows="2" placeholder="例如: nginx -s reload" />
      </FormItem>

      <Alert
        type="info"
        show-icon
        message="添加后不会立即颁发证书；可点击「立即续期」手动颁发，或等待到达续期时间后由调度器自动颁发。"
      />
    </Form>
  </Modal>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  Alert,
  Form,
  FormItem,
  Input,
  InputNumber,
  Modal,
  Select,
  TextArea,
  message,
  type FormInstance,
  type Rule,
} from "antdv-next";
import { api } from "../api";
import type { CertItem, CertPayload, MetaInfo, ProviderItem } from "../api/types";

const props = defineProps<{
  open: boolean;
  cert: CertItem | null;
  providers: ProviderItem[];
  meta: MetaInfo | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
  (e: "saved"): void;
}>();

const formRef = ref<FormInstance>();
const saving = ref(false);

const caOptions = computed(() =>
  (props.meta?.cas || ["Let's Encrypt"]).map((c) => ({ label: c, value: c })),
);

const providerOptions = computed(() =>
  props.providers.map((p) => ({
    label: `${p.type_label} / ${p.name}${p.enabled ? "" : "（已停用）"}`,
    value: p.id,
    disabled: !p.enabled,
  })),
);

const form = reactive({
  name: "",
  provider_id: undefined as number | undefined,
  domains: [] as string[],
  cert_path: "",
  key_path: "",
  ca: "Let's Encrypt",
  renewal_days: 60,
  post_command: "",
});

const domainRe = /^(\*\.)?([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$/i;

const rules: Record<string, Rule[]> = {
  provider_id: [{ required: true, message: "请选择 DNS 运营商配置" }],
  domains: [
    {
      required: true,
      validator: (_r: unknown, v: string[]) => {
        if (!v || v.length === 0) return Promise.reject("至少填写一个域名");
        const bad = v.find((d) => !domainRe.test(d.trim()));
        if (bad) return Promise.reject(`域名格式不正确: ${bad}`);
        if (new Set(v.map((d) => d.trim().toLowerCase())).size !== v.length) {
          return Promise.reject("存在重复域名");
        }
        return Promise.resolve();
      },
    },
  ],
  cert_path: [{ required: true, message: "请填写公钥位置（绝对路径）" }],
  key_path: [{ required: true, message: "请填写私钥位置（绝对路径）" }],
  renewal_days: [{ required: true, message: "请填写续期周期" }],
};

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const c = props.cert;
    if (c) {
      form.name = c.name;
      form.provider_id = c.provider_id;
      form.domains = [...c.domains];
      form.cert_path = c.cert_path;
      form.key_path = c.key_path;
      form.ca = c.ca;
      form.renewal_days = c.renewal_days;
      form.post_command = c.post_command;
    } else {
      form.name = "";
      form.provider_id = undefined;
      form.domains = [];
      form.cert_path = "";
      form.key_path = "";
      form.ca = props.meta?.ca_default || "Let's Encrypt";
      form.renewal_days = 60;
      form.post_command = "";
    }
  },
);

const close = () => emit("update:open", false);

const onOk = async () => {
  try {
    await formRef.value?.validate();
  } catch {
    return;
  }
  const payload: CertPayload = {
    name: form.name.trim(),
    provider_id: form.provider_id!,
    domains: form.domains.map((d) => d.trim()).filter(Boolean),
    cert_path: form.cert_path.trim(),
    key_path: form.key_path.trim(),
    ca: form.ca,
    renewal_days: form.renewal_days,
    post_command: form.post_command.trim(),
  };
  saving.value = true;
  try {
    if (props.cert) {
      await api.updateCert(props.cert.id, payload);
      message.success("证书配置已更新");
    } else {
      await api.createCert(payload);
      message.success("证书配置已添加，可点击「立即续期」颁发");
    }
    emit("saved");
    close();
  } catch {
    // 错误提示由拦截器统一处理
  } finally {
    saving.value = false;
  }
};
</script>
