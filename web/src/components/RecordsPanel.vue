<template>
  <div>
    <div class="toolbar">
      <Space wrap>
        <Select
          v-model:value="filterCertId"
          allow-clear
          placeholder="全部证书"
          style="width: 240px"
          :options="certOptions"
          @change="reload"
        />
        <Select
          v-model:value="filterSuccess"
          allow-clear
          placeholder="全部结果"
          style="width: 120px"
          :options="[
            { label: '成功', value: 1 },
            { label: '失败', value: 0 },
          ]"
          @change="reload"
        />
        <Popconfirm
          :title="`确定删除选中的 ${selectedIds.length} 条颁发记录？`"
          description="删除后不可恢复"
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
        <Button @click="reload">
          <span class="btn-inner"><RefreshCw :size="14" /> 刷新</span>
        </Button>
      </Space>
      <span class="sub-text">共 {{ total }} 条记录（包含成功与失败）</span>
    </div>

    <Table
      :columns="columns"
      :data-source="list"
      :loading="loading"
      :row-selection="rowSelection"
      row-key="id"
      size="middle"
      :scroll="{ x: 1560 }"
      :pagination="{
        current: page,
        pageSize: pageSize,
        total: total,
        showTotal: (t: number) => `共 ${t} 条`,
        showSizeChanger: false,
      }"
      @change="onTableChange"
    >
      <template #bodyCell="{ column, record }">
        <template v-if="column.key === 'issued_at'">
          <span class="mono-time">{{ record.issued_at }}</span>
        </template>
        <template v-else-if="column.key === 'expire'">
          <span class="mono-time">{{ record.expire_at || "—" }}</span>
        </template>
        <template v-else-if="column.key === 'provider'">
          <Tooltip :title="record.provider_name ? `配置: ${record.provider_name}` : ''">
            <Tag :color="typeColor(record.provider_type)">{{ record.provider_label }}</Tag>
          </Tooltip>
        </template>
        <template v-else-if="column.key === 'domains'">
          <Tooltip :title="record.domains.join(', ')">
            <span class="path-text">{{ record.domains.join(", ") }}</span>
          </Tooltip>
        </template>
        <template v-else-if="column.key === 'cert_path'">
          <span class="path-text">{{ record.cert_path }}</span>
        </template>
        <template v-else-if="column.key === 'key_path'">
          <span class="path-text">{{ record.key_path }}</span>
        </template>
        <template v-else-if="column.key === 'trigger'">
          <Tag v-if="record.trigger === 'manual'" color="blue">手动</Tag>
          <Tag v-else color="cyan">自动</Tag>
        </template>
        <template v-else-if="column.key === 'success'">
          <Tooltip :title="record.message || ''">
            <Tag v-if="record.success" color="green">成功</Tag>
            <Tag v-else color="red">失败</Tag>
          </Tooltip>
        </template>
        <template v-else-if="column.key === 'action'">
          <Space :size="2" wrap>
            <Button type="link" size="small" @click="showDetail(record as RecordItem)">
              <span class="btn-inner"><FileText :size="13" /> 详情</span>
            </Button>
            <Popconfirm
              :title="`确定删除颁发记录 #${(record as RecordItem).id}？`"
              description="删除后不可恢复"
              ok-text="删除"
              ok-type="danger"
              @confirm="onDelete(record as RecordItem)"
            >
              <Button type="link" size="small" danger>
                <span class="btn-inner"><Trash2 :size="13" /> 删除</span>
              </Button>
            </Popconfirm>
          </Space>
        </template>
      </template>
    </Table>

    <Modal
      :open="detailOpen"
      :title="detail ? `颁发记录 #${detail.id}` : '颁发记录'"
      :width="780"
      :footer="null"
      @cancel="detailOpen = false"
    >
      <Descriptions v-if="detail" :column="2" size="small" bordered style="margin-top: 8px">
        <DescriptionsItem label="颁发时间">{{ detail.issued_at }}</DescriptionsItem>
        <DescriptionsItem label="到期时间">{{ detail.expire_at || "—" }}</DescriptionsItem>
        <DescriptionsItem label="触发方式">
          {{ detail.trigger === "manual" ? "手动" : "自动" }}
        </DescriptionsItem>
        <DescriptionsItem label="颁发结果">
          <Tag v-if="detail.success" color="green">成功</Tag>
          <Tag v-else color="red">失败</Tag>
        </DescriptionsItem>
        <DescriptionsItem label="DNS 运营商">
          {{ detail.provider_label }}{{ detail.provider_name ? ` / ${detail.provider_name}` : "" }}
        </DescriptionsItem>
        <DescriptionsItem label="CA 机构">{{ detail.ca }}</DescriptionsItem>
        <DescriptionsItem label="域名" :span="2">
          {{ detail.domains.join(", ") }}
        </DescriptionsItem>
        <DescriptionsItem label="公钥位置">{{ detail.cert_path }}</DescriptionsItem>
        <DescriptionsItem label="私钥位置">{{ detail.key_path }}</DescriptionsItem>
        <DescriptionsItem label="说明" :span="2">{{ detail.message || "—" }}</DescriptionsItem>
      </Descriptions>
      <Divider style="margin: 14px 0" title-placement="left">命令执行结果</Divider>
      <pre class="cmd-output">{{ detail?.command_output || "（无输出）" }}</pre>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Button,
  Descriptions,
  DescriptionsItem,
  Divider,
  Modal,
  Popconfirm,
  Space,
  Select,
  Spin,
  Table,
  Tag,
  Tooltip,
  message,
  type TableColumnsType,
} from "antdv-next";
import { FileText, RefreshCw, Trash2 } from "@lucide/vue";
import { api } from "../api";
import type { RecordDetail, RecordItem } from "../api/types";

const props = defineProps<{ certs: { id: number; name: string; domains: string[] }[] }>();

const list = ref<RecordItem[]>([]);
const loading = ref(false);
const page = ref(1);
const pageSize = 20;
const total = ref(0);
const filterCertId = ref<number | undefined>(undefined);
const filterSuccess = ref<number | undefined>(undefined);
const detailOpen = ref(false);
const detail = ref<RecordDetail | null>(null);
const selectedIds = ref<number[]>([]);

const rowSelection = computed(() => ({
  selectedRowKeys: selectedIds.value,
  onChange: (keys: (string | number)[]) => {
    selectedIds.value = keys.map(Number);
  },
}));

const certOptions = computed(() =>
  props.certs.map((c) => ({
    label: c.name || c.domains.join(", "),
    value: c.id,
  })),
);

const typeColor = (t: string) =>
  (
    ({ ali: "orange", tencent: "blue", huaweicloud: "red", hw: "red", cf: "volcano" }) as Record<
      string,
      string
    >
  )[t] || "default";

const columns: TableColumnsType = [
  { title: "颁发时间", key: "issued_at", width: 165 },
  { title: "到期时间", key: "expire", width: 165 },
  { title: "DNS 运营商", key: "provider", width: 110 },
  { title: "域名", key: "domains", width: 220, ellipsis: true },
  { title: "公钥位置", key: "cert_path", width: 190, ellipsis: true },
  { title: "私钥位置", key: "key_path", width: 190, ellipsis: true },
  { title: "CA 机构", dataIndex: "ca", key: "ca", width: 125, ellipsis: true },
  { title: "触发", key: "trigger", width: 76 },
  { title: "结果", key: "success", width: 80 },
  { title: "操作", key: "action", width: 185, fixed: "right" },
];

const reload = async () => {
  loading.value = true;
  try {
    const res = await api.records({
      page: page.value,
      page_size: pageSize,
      cert_id: filterCertId.value,
      success: filterSuccess.value === undefined ? undefined : filterSuccess.value === 1,
    });
    list.value = res.list;
    total.value = res.total;
  } catch {
    // 错误提示由拦截器统一处理
  } finally {
    loading.value = false;
  }
};

const onTableChange = (pag: { current?: number }) => {
  if (pag?.current) page.value = pag.current;
  void reload();
};

const showDetail = async (r: RecordItem) => {
  try {
    detail.value = await api.recordDetail(r.id);
    detailOpen.value = true;
  } catch {
    // 错误提示由拦截器统一处理
  }
};

// 删除后若当前页被删空且不是第一页，回退一页
const backPageIfEmpty = (removedOnPage: number) => {
  if (removedOnPage === list.value.length && page.value > 1) page.value -= 1;
};

const onDelete = async (r: RecordItem) => {
  try {
    await api.deleteRecord(r.id);
    message.success("已删除");
    backPageIfEmpty(1);
    selectedIds.value = selectedIds.value.filter((id) => id !== r.id);
    await reload();
  } catch {
    // 错误提示由拦截器统一处理
  }
};

const onBatchDelete = async () => {
  try {
    const res = await api.batchDeleteRecords(selectedIds.value);
    message.success(`已删除 ${res.deleted} 项`);
    const sel = selectedIds.value;
    backPageIfEmpty(list.value.filter((r) => sel.includes(r.id)).length);
    selectedIds.value = [];
    await reload();
  } catch {
    // 错误提示由拦截器统一处理
  }
};

onMounted(reload);

defineExpose({ reload });
</script>

<style lang="scss" scoped>
.mono-time {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
}

.btn-inner {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
</style>
