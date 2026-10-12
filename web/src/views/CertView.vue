<template>
  <div class="page">
    <div class="card-panel">
      <Tabs v-model:active-key="tab">
        <TabPane key="list" tab="证书列表">
          <div class="toolbar">
            <Space wrap>
              <Button type="primary" @click="openCreate">
                <span class="btn-inner"><Plus :size="14" /> 添加证书</span>
              </Button>
              <Popconfirm
                title="确定删除选中的证书配置？"
                description="删除后不可恢复（历史颁发记录将保留）"
                ok-text="删除"
                ok-type="danger"
                :disabled="selectedIds.length === 0"
                @confirm="onBatchDelete"
              >
                <Button danger :disabled="selectedIds.length === 0">
                  <span class="btn-inner">
                    <Trash2 :size="14" />
                    批量删除{{ selectedIds.length ? ` (${selectedIds.length})` : "" }}
                  </span>
                </Button>
              </Popconfirm>
              <Button @click="refresh">
                <span class="btn-inner"><RefreshCw :size="14" /> 刷新</span>
              </Button>
              <Dropdown :trigger="['click']">
                <Button>
                  <span class="btn-inner"><Columns3 :size="14" /> 列设置</span>
                </Button>
                <template #popupRender>
                  <div class="float-panel column-settings">
                    <Checkbox :checked="colVisible.cert_path" @change="onCertPathChange">
                      公钥位置
                    </Checkbox>
                    <Checkbox :checked="colVisible.key_path" @change="onKeyPathChange">
                      私钥位置
                    </Checkbox>
                  </div>
                </template>
              </Dropdown>
            </Space>
            <InputSearch
              v-model:value="keyword"
              placeholder="搜索名称 / 域名 / 路径"
              allow-clear
              style="width: 260px"
            />
          </div>

          <Alert
            v-if="providers.length === 0"
            type="info"
            show-icon
            message="尚未配置 DNS 运营商"
            style="margin-bottom: 16px"
          >
            <template #description>
              添加证书前需要先配置 DNS 运营商的接口凭据（阿里云 / 腾讯云 / 华为云 / Cloudflare）。
              <a @click="router.push(`/dns/${firstMainstreamKey}`)">前往配置</a>
            </template>
          </Alert>

          <Table
            :columns="columns"
            :data-source="filtered"
            :loading="loading"
            :row-selection="rowSelection"
            row-key="id"
            size="middle"
            :scroll="{ x: scrollX }"
            :pagination="false"
          >
            <template #bodyCell="{ column, record }">
              <template v-if="column.key === 'name'">
                <span v-if="record.name">{{ record.name }}</span>
                <span v-else class="sub-text">—</span>
              </template>
              <template v-else-if="column.key === 'provider'">
                <Tooltip
                  v-if="record.provider_type"
                  :title="`dns hook: ${dnsHook(record.provider_type)}`"
                >
                  <Tag :color="typeColor(record.provider_type)">
                    {{ providerLabel(record.provider_type) }}
                  </Tag>
                </Tooltip>
                <Tag v-else>未知</Tag>
                <div class="sub-text">{{ record.provider_name || "-" }}</div>
              </template>
              <template v-else-if="column.key === 'domains'">
                <div class="domain-list">
                  <Tooltip v-for="d in record.domains" :key="d" :title="d">
                    <Tag class="domain-tag">{{ d }}</Tag>
                  </Tooltip>
                </div>
              </template>
              <template v-else-if="column.key === 'status'">
                <span v-if="record.status === 'issuing'" class="status-cell">
                  <Spin size="small" /> 颁发中
                </span>
                <Badge v-else-if="record.status === 'issued'" status="success" text="已颁发" />
                <Badge v-else-if="record.status === 'pending'" status="processing" text="待颁发" />
                <Tooltip v-else :title="record.last_error || '未知错误'">
                  <Badge status="error" text="失败" />
                </Tooltip>
              </template>
              <template v-else-if="column.key === 'expire'">
                <span class="mono-time">{{ record.cert_expire_at || "—" }}</span>
              </template>
              <template v-else-if="column.key === 'next'">
                <span v-if="record.status === 'issuing'" class="sub-text">—</span>
                <span v-else class="mono-time">{{ record.next_renew_at || "—" }}</span>
              </template>
              <template v-else-if="column.key === 'cert_path'">
                <span class="path-text">{{ record.cert_path }}</span>
              </template>
              <template v-else-if="column.key === 'key_path'">
                <span class="path-text">{{ record.key_path }}</span>
              </template>
              <template v-else-if="column.key === 'action'">
                <Space :size="2" wrap>
                  <Button
                    type="link"
                    size="small"
                    :disabled="record.status === 'issuing'"
                    @click="onRenew(record)"
                  >
                    <span class="btn-inner"><Zap :size="13" /> 立即续期</span>
                  </Button>
                  <Button type="link" size="small" @click="openEdit(record)">
                    <span class="btn-inner"><Pencil :size="13" /> 编辑</span>
                  </Button>
                  <Popconfirm
                    :title="`确定删除证书「${displayName(record)}」？`"
                    ok-text="删除"
                    ok-type="danger"
                    @confirm="onDelete(record)"
                  >
                    <Button type="link" size="small" danger>
                      <span class="btn-inner"><Trash2 :size="13" /> 删除</span>
                    </Button>
                  </Popconfirm>
                </Space>
              </template>
            </template>
          </Table>
        </TabPane>
        <TabPane key="records" tab="颁发记录">
          <RecordsPanel :certs="list" />
        </TabPane>
      </Tabs>
    </div>

    <CertFormModal
      v-model:open="formOpen"
      :cert="editing"
      :providers="providers"
      :meta="meta"
      @saved="refresh"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import {
  Alert,
  Badge,
  Button,
  Checkbox,
  Dropdown,
  InputSearch,
  Popconfirm,
  Space,
  Spin,
  Table,
  TabPane,
  Tabs,
  Tag,
  Tooltip,
  message,
  type TableColumnsType,
} from "antdv-next";
import { Columns3, Pencil, Plus, RefreshCw, Trash2, Zap } from "@lucide/vue";
import CertFormModal from "../components/CertFormModal.vue";
import RecordsPanel from "../components/RecordsPanel.vue";
import { api, getProviderSchemas } from "../api";
import type { CertItem, MetaInfo, ProviderItem, ProviderSchema } from "../api/types";

const router = useRouter();
const tab = ref("list");
const list = ref<CertItem[]>([]);
const providers = ref<ProviderItem[]>([]);
const schemas = ref<ProviderSchema[]>([]);
const meta = ref<MetaInfo | null>(null);
const loading = ref(false);
const keyword = ref("");
const selectedIds = ref<number[]>([]);
const formOpen = ref(false);
const editing = ref<CertItem | null>(null);
let pollTimer: number | undefined;

// 公钥/私钥位置列默认隐藏，通过工具栏「列设置」勾选显示，选择持久化到 localStorage
const COLUMNS_PREF_KEY = "acme-webui:cert-columns";
const colVisible = reactive({ cert_path: false, key_path: false });
try {
  const raw = localStorage.getItem(COLUMNS_PREF_KEY);
  if (raw) {
    const p = JSON.parse(raw) as Partial<typeof colVisible>;
    colVisible.cert_path = !!p.cert_path;
    colVisible.key_path = !!p.key_path;
  }
} catch {
  // 忽略
}

const columnMenuChange = (k: keyof typeof colVisible, checked: boolean) => {
  colVisible[k] = checked;
  try {
    localStorage.setItem(COLUMNS_PREF_KEY, JSON.stringify(colVisible));
  } catch {
    // 忽略
  }
};
const onCertPathChange = (e: { target: { checked: boolean } }) =>
  columnMenuChange("cert_path", e.target.checked);
const onKeyPathChange = (e: { target: { checked: boolean } }) =>
  columnMenuChange("key_path", e.target.checked);

const columns = computed<TableColumnsType>(() => [
  { title: "名称", key: "name", width: 110, ellipsis: true },
  { title: "DNS 运营商", key: "provider", width: 118 },
  { title: "域名", key: "domains", width: 225 },
  { title: "状态", key: "status", width: 98 },
  { title: "证书到期", key: "expire", width: 160 },
  { title: "下次续期", key: "next", width: 160 },
  { title: "周期(天)", dataIndex: "renewal_days", key: "days", width: 86 },
  ...(colVisible.cert_path
    ? [{ title: "公钥位置", key: "cert_path", width: 185, ellipsis: true }]
    : []),
  ...(colVisible.key_path
    ? [{ title: "私钥位置", key: "key_path", width: 185, ellipsis: true }]
    : []),
  { title: "CA 机构", dataIndex: "ca", key: "ca", width: 118, ellipsis: true },
  { title: "操作", key: "action", width: 232, fixed: "right" },
]);

const scrollX = computed(() => columns.value.reduce((s, c) => s + ((c.width as number) || 0), 0));

const filtered = computed(() => {
  const k = keyword.value.trim().toLowerCase();
  if (!k) return list.value;
  return list.value.filter(
    (c) =>
      c.name.toLowerCase().includes(k) ||
      c.domains.some((d) => d.toLowerCase().includes(k)) ||
      c.cert_path.toLowerCase().includes(k) ||
      c.key_path.toLowerCase().includes(k),
  );
});

const rowSelection = computed(() => ({
  selectedRowKeys: selectedIds.value,
  onChange: (keys: (string | number)[]) => {
    selectedIds.value = keys.map(Number);
  },
  getCheckboxProps: (r: CertItem) => ({ disabled: r.status === "issuing" }),
}));

const displayName = (r: CertItem) => r.name || r.domains.join(",");
const providerLabel = (t: string) => schemas.value.find((s) => s.key === t)?.label || t;
const dnsHook = (t: string) => schemas.value.find((s) => s.key === t)?.dnsHook || "";
const firstMainstreamKey = computed(
  () => schemas.value.find((s) => s.category === "mainstream")?.key || "ali",
);
const typeColor = (t: string | null) =>
  (
    ({ ali: "orange", tencent: "blue", huaweicloud: "red", hw: "red", cf: "volcano" }) as Record<
      string,
      string
    >
  )[t || ""] || "default";

const fetchList = async () => {
  loading.value = true;
  try {
    list.value = (await api.certs()) || [];
    schedulePoll();
  } finally {
    loading.value = false;
  }
};

const refresh = async () => {
  await Promise.all([fetchList(), loadProviders()]);
};

const loadProviders = async () => {
  try {
    providers.value = (await api.providers()) || [];
  } catch {
    // 忽略
  }
};

// 存在颁发中的证书时，3 秒轮询刷新状态
const schedulePoll = () => {
  const any = list.value.some((c) => c.status === "issuing");
  if (any && !pollTimer) {
    pollTimer = window.setInterval(() => void fetchList(), 3000);
  } else if (!any && pollTimer) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
};

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});

const openCreate = () => {
  editing.value = null;
  formOpen.value = true;
};

const openEdit = (r: CertItem) => {
  editing.value = r;
  formOpen.value = true;
};

const onRenew = async (r: CertItem) => {
  try {
    await api.renewCert(r.id);
    message.success("已开始颁发，请稍后查看结果");
    await fetchList();
  } catch {
    // 错误提示由拦截器统一处理
  }
};

const onDelete = async (r: CertItem) => {
  try {
    await api.deleteCert(r.id);
    message.success("已删除");
    selectedIds.value = selectedIds.value.filter((id) => id !== r.id);
    await fetchList();
  } catch {
    // 错误提示由拦截器统一处理
  }
};

const onBatchDelete = async () => {
  try {
    const res = await api.batchDeleteCerts(selectedIds.value);
    message.success(`已删除 ${res.deleted} 项`);
    selectedIds.value = [];
    await fetchList();
  } catch {
    // 错误提示由拦截器统一处理
  }
};

onMounted(async () => {
  await refresh();
  try {
    schemas.value = await getProviderSchemas();
  } catch {
    // 忽略
  }
  try {
    meta.value = await api.meta();
  } catch {
    // 忽略
  }
});
</script>

<style lang="scss" scoped>
.domain-list {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;

  .domain-tag {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
  }
}

.column-settings {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px 14px;
  min-width: 130px;
}

.status-cell {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--ant-color-primary, #1677ff);
}

.mono-time {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
  white-space: nowrap;
}

.btn-inner {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
</style>
