<template>
  <div class="page">
    <div v-if="def" class="card-panel">
      <div class="prov-head">
        <div class="prov-title">
          <span class="prov-name">{{ def.label }}</span>
          <span class="prov-en">{{ def.labelEn }}</span>
          <Tag color="blue">acme.sh: {{ def.dnsHook }}</Tag>
          <Tag>{{ def.category === "mainstream" ? "主流 DNS" : "其他 DNS" }}</Tag>
        </div>
        <a :href="def.docUrl" target="_blank" rel="noreferrer">
          <span class="btn-inner">查看 wiki 文档 <ExternalLink :size="13" /></span>
        </a>
      </div>

      <Alert
        v-if="def.description"
        type="info"
        show-icon
        :message="def.description"
        style="margin-bottom: 16px"
      />

      <Alert
        v-if="meta && !meta.acme_sh.found"
        type="warning"
        show-icon
        message="acme.sh 未就绪，完成配置前请先安装"
        style="margin-bottom: 16px; cursor: pointer"
        @click="installOpen = true"
      />

      <Form layout="vertical">
        <template v-if="def.fields.length">
          <div class="form-grid" v-if="gridFields.length">
            <FormItem
              v-for="f in gridFields"
              :key="f.key"
              :name="`cred_${f.key}`"
              :extra="f.help || undefined"
            >
              <template #label>
                <span :class="{ 'env-key-label': f.label === f.key }">
                  {{ f.label }}{{ f.required ? "" : "（可选）" }}
                </span>
              </template>
              <InputPassword
                v-if="f.secret"
                v-model:value="creds[f.key]"
                :placeholder="f.placeholder || f.key"
                autocomplete="new-password"
              />
              <Input v-else v-model:value="creds[f.key]" :placeholder="f.placeholder || f.key" />
            </FormItem>
          </div>
          <FormItem
            v-for="f in restFields"
            :key="f.key"
            :name="`cred_${f.key}`"
            :extra="f.help || undefined"
          >
            <template #label>
              <span :class="{ 'env-key-label': f.label === f.key }">
                {{ f.label }}{{ f.required ? "" : "（可选）" }}
              </span>
            </template>
            <InputPassword
              v-if="f.secret"
              v-model:value="creds[f.key]"
              :placeholder="f.placeholder || f.key"
              autocomplete="new-password"
            />
            <Input v-else v-model:value="creds[f.key]" :placeholder="f.placeholder || f.key" />
          </FormItem>
        </template>
        <Alert
          v-else
          type="info"
          show-icon
          message="该运营商没有需要预设的凭据字段，如有需要请在下方「其他环境变量」中填写"
          style="margin-bottom: 16px"
        />

        <div class="extra-section">
          <div class="extra-title">
            其他环境变量
            <span class="sub-text">（可选，补充 acme.sh 该插件支持的其他环境变量）</span>
          </div>
          <div v-for="(e, i) in extras" :key="i" class="extra-row">
            <Input v-model:value="e.key" placeholder="变量名，如 CF_Zone_ID" class="extra-key" />
            <Input v-model:value="e.value" placeholder="值" />
            <Button type="text" danger @click="extras.splice(i, 1)">
              <Trash2 :size="14" />
            </Button>
          </div>
          <Button size="small" @click="extras.push({ key: '', value: '' })">
            <span class="btn-inner"><Plus :size="13" /> 添加变量</span>
          </Button>
        </div>

        <FormItem label="启用" name="enabled">
          <Switch v-model:checked="enabled" />
          <span class="sub-text" style="margin-left: 10px">停用后相关证书将暂停自动续期</span>
        </FormItem>
      </Form>

      <div class="foot">
        <span v-if="existing" class="sub-text"> 已配置（{{ existing.updated_at }} 更新） </span>
        <span v-else class="sub-text">尚未配置</span>
        <Space>
          <Popconfirm
            v-if="existing"
            title="确定删除该配置？删除后关联证书无法颁发"
            ok-text="删除"
            ok-type="danger"
            @confirm="onDelete"
          >
            <Button danger>
              <span class="btn-inner"><Trash2 :size="14" /> 删除配置</span>
            </Button>
          </Popconfirm>
          <Button type="primary" :loading="saving" @click="onSave">
            <span class="btn-inner"><Check :size="14" /> 保存配置</span>
          </Button>
        </Space>
      </div>
    </div>

    <div v-else-if="loaded" class="card-panel">
      <Empty description="未知的 DNS 运营商，请从左侧菜单选择" />
    </div>

    <AcmeInstallModal
      v-model:open="installOpen"
      :install="meta?.acme_sh.install || null"
      @redetect="loadMeta"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute } from "vue-router";
import {
  Alert,
  Button,
  Empty,
  Form,
  FormItem,
  Input,
  InputPassword,
  Popconfirm,
  Space,
  Switch,
  Tag,
  message,
} from "antdv-next";
import { Check, ExternalLink, Plus, Trash2 } from "@lucide/vue";
import AcmeInstallModal from "../components/AcmeInstallModal.vue";
import { api, getProviderSchemas } from "../api";
import type { MetaInfo, ProviderItem, ProviderSchema } from "../api/types";

const route = useRoute();
const schemas = ref<ProviderSchema[]>([]);
const providers = ref<ProviderItem[]>([]);
const meta = ref<MetaInfo | null>(null);
const loaded = ref(false);
const saving = ref(false);
const enabled = ref(true);
const installOpen = ref(false);
const creds = reactive<Record<string, string>>({});
const extras = ref<{ key: string; value: string }[]>([]);

const typeKey = () => String(route.params.type || "");

const def = computed<ProviderSchema | null>(
  () => schemas.value.find((s) => s.key === typeKey()) || null,
);
const existing = computed<ProviderItem | null>(
  () => providers.value.find((p) => p.provider_type === typeKey()) || null,
);

const gridFields = computed(() => def.value?.fields.slice(0, 2) || []);
const restFields = computed(() => def.value?.fields.slice(2) || []);

const applyExisting = () => {
  const d = def.value;
  if (!d) return;
  const c = existing.value?.credentials || {};
  const known = new Set(d.fields.map((f) => f.key));
  for (const k of Object.keys(creds)) delete creds[k];
  extras.value = [];
  for (const f of d.fields) {
    creds[f.key] = c[f.key] || "";
  }
  for (const [k, v] of Object.entries(c)) {
    if (!known.has(k)) extras.value.push({ key: k, value: v });
  }
  enabled.value = existing.value?.enabled ?? true;
};

const loadAll = async () => {
  try {
    schemas.value = await getProviderSchemas();
  } catch {
    // 忽略
  }
  try {
    providers.value = (await api.providers()) || [];
  } catch {
    // 忽略
  }
  try {
    meta.value = await api.meta();
  } catch {
    // 忽略
  }
  loaded.value = true;
  applyExisting();
  if (def.value) {
    document.title = `${def.value.label} - ACME WebUI`;
  }
};

const loadMeta = async () => {
  try {
    meta.value = await api.meta();
    if (meta.value?.acme_sh.found) message.success("已检测到 acme.sh");
  } catch {
    // 忽略
  }
};

const ENV_KEY_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;

const collectCreds = (): Record<string, string> | null => {
  const d = def.value;
  if (!d) return null;
  const out: Record<string, string> = {};
  for (const f of d.fields) {
    const v = (creds[f.key] || "").trim();
    if (v) out[f.key] = v;
  }
  for (const e of extras.value) {
    const k = e.key.trim();
    const v = e.value.trim();
    if (!k) continue;
    if (!ENV_KEY_RE.test(k)) {
      message.error(`非法的环境变量名: ${k}`);
      return null;
    }
    if (v) {
      if (out[k]) {
        message.error(`环境变量 ${k} 重复`);
        return null;
      }
      out[k] = v;
    }
  }
  for (const f of d.fields) {
    if (f.required && !out[f.key]) {
      message.error(`请填写 ${f.label === f.key ? f.key : f.label}`);
      return null;
    }
  }
  if (d.requireAny?.length && !d.requireAny.some((g) => g.every((k) => out[k]))) {
    message.error("凭据不完整，需提供: " + d.requireAny.map((g) => g.join(" + ")).join(" 或 "));
    return null;
  }
  if (!d.noCreds && Object.keys(out).length === 0) {
    message.error("请至少填写一项凭据（或添加其他环境变量）");
    return null;
  }
  return out;
};

const onSave = async () => {
  const d = def.value;
  if (!d) return;
  const credentials = collectCreds();
  if (!credentials) return;
  saving.value = true;
  try {
    await api.saveProvider({
      provider_type: d.key,
      credentials,
      enabled: enabled.value,
    });
    message.success(`${d.label} 配置已保存`);
    await loadAll();
  } catch {
    // 错误提示由拦截器统一处理
  } finally {
    saving.value = false;
  }
};

const onDelete = async () => {
  const x = existing.value;
  if (!x) return;
  try {
    await api.deleteProvider(x.id);
    message.success("配置已删除");
    await loadAll();
  } catch {
    // 错误提示由拦截器统一处理（如被证书占用）
  }
};

watch(
  () => route.params.type,
  () => {
    loaded.value = false;
    void loadAll();
  },
);

onMounted(loadAll);
</script>

<style lang="scss" scoped>
.prov-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;

  .prov-title {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }

  .prov-name {
    font-size: 18px;
    font-weight: 600;
    color: var(--ant-color-text, #1f2937);
  }

  .prov-en {
    font-size: 13px;
    color: var(--ant-color-text-tertiary, #9ca3af);
  }
}

.env-key-label {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  column-gap: 20px;
}

.extra-section {
  border: 1px dashed var(--ant-color-border, #e5e7eb);
  border-radius: 8px;
  padding: 12px 14px;
  margin-bottom: 20px;

  .extra-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--ant-color-text-secondary, #374151);
    margin-bottom: 10px;
  }

  .extra-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;

    .extra-key {
      width: 280px;
      flex: none;
    }
  }
}

.foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 14px;
  margin-top: 8px;
  padding-top: 16px;
  border-top: 1px solid var(--ant-color-border-secondary, #f0f0f0);
}

.btn-inner {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
</style>
