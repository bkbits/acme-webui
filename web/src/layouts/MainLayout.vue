<template>
  <Layout class="main-layout">
    <LayoutSider
      v-model:collapsed="collapsed"
      class="sider"
      :width="208"
      :collapsed-width="64"
      breakpoint="lg"
      collapsible
    >
      <div class="brand">
        <span class="brand-icon"><ShieldCheck :size="18" :stroke-width="2.2" /></span>
        <span v-show="!collapsed" class="brand-name">ACME WebUI</span>
      </div>
      <Menu
        v-model:selectedKeys="selectedKeys"
        v-model:openKeys="openKeys"
        mode="inline"
        class="sider-menu"
        @click="onMenuClick"
      >
        <MenuItem key="certs">
          <span class="menu-item-inner"><ShieldCheck class="menu-icon" :size="15" />证书管理</span>
        </MenuItem>
        <SubMenu key="dns-mainstream">
          <template #title>
            <span class="menu-item-inner"><Server class="menu-icon" :size="15" />主流 DNS</span>
          </template>
          <MenuItem v-for="s in mainstreamSchemas" :key="`dns/${s.key}`">
            <span class="menu-item-inner">{{ s.label }}</span>
          </MenuItem>
        </SubMenu>
        <SubMenu key="dns-other">
          <template #title>
            <span class="menu-item-inner"><Globe class="menu-icon" :size="15" />其他 DNS</span>
          </template>
          <MenuItem v-for="s in otherSchemas" :key="`dns/${s.key}`">
            <span class="menu-item-inner">{{ s.label }}</span>
          </MenuItem>
        </SubMenu>
      </Menu>
    </LayoutSider>
    <Layout>
      <LayoutHeader class="header">
        <div class="header-left">
          <span class="app-title">acme 证书自动申请续期服务平台</span>
        </div>
        <div class="header-right">
          <template v-if="meta">
            <Tooltip v-if="meta.acme_sh.found" :title="`acme.sh 路径: ${meta.acme_sh.path}`">
              <span class="acme-chip ok">
                <span class="acme-dot"></span>
                acme.sh 就绪
              </span>
            </Tooltip>
            <Tooltip v-else title="点击查看安装指引">
              <span class="acme-chip bad clickable" @click="installOpen = true">
                <span class="acme-dot"></span>
                acme.sh 未就绪
              </span>
            </Tooltip>
          </template>
          <span
            class="user-chip"
            :title="mode === 'dark' ? '切换到亮色主题' : '切换到暗色主题'"
            @click="toggle"
          >
            <Sun v-if="mode === 'dark'" :size="14" />
            <Moon v-else :size="14" />
          </span>
          <Dropdown :menu="{ items: userMenuItems, onClick: onUserMenu }">
            <span class="user-chip">
              <User :size="14" />
              {{ auth.username.value || "admin" }}
              <ChevronDown :size="13" />
            </span>
          </Dropdown>
        </div>
      </LayoutHeader>
      <LayoutContent class="content">
        <RouterView />
      </LayoutContent>
    </Layout>
  </Layout>
  <ChangePasswordModal v-model:open="showPwd" />
  <AcmeInstallModal
    v-model:open="installOpen"
    :install="meta?.acme_sh.install || null"
    @redetect="redetectAcme"
  />
</template>

<script setup lang="ts">
import { computed, h, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Dropdown,
  Layout,
  LayoutContent,
  LayoutHeader,
  LayoutSider,
  Menu,
  MenuItem,
  SubMenu,
  Tooltip,
  message,
} from "antdv-next";
import {
  ChevronDown,
  Globe,
  KeyRound,
  LogOut,
  Moon,
  Server,
  ShieldCheck,
  Sun,
  User,
} from "@lucide/vue";
import AcmeInstallModal from "../components/AcmeInstallModal.vue";
import ChangePasswordModal from "../components/ChangePasswordModal.vue";
import { api, getProviderSchemas, type MetaInfo } from "../api";
import type { ProviderSchema } from "../api/types";
import { useAuth } from "../composables/auth";
import { useTheme } from "../composables/theme";

const route = useRoute();
const router = useRouter();
const auth = useAuth();
const { mode, toggle } = useTheme();

const collapsed = ref(false);
const selectedKeys = ref<string[]>(["certs"]);
const openKeys = ref<string[]>([]);
const meta = ref<MetaInfo | null>(null);
const schemas = ref<ProviderSchema[]>([]);
const showPwd = ref(false);
const installOpen = ref(false);

// 主流 DNS 固定顺序（与 acme.sh wiki 主流提供商列表一致）
const MAINSTREAM_ORDER = [
  "ali",
  "cf",
  "aws",
  "hw",
  "netcup",
  "azure",
  "dnsmint",
  "opteamax",
  "optidata",
];

const mainstreamSchemas = computed(() =>
  schemas.value
    .filter((s) => s.category === "mainstream")
    .sort((a, b) => {
      const ia = MAINSTREAM_ORDER.indexOf(a.key);
      const ib = MAINSTREAM_ORDER.indexOf(b.key);
      return (ia === -1 ? 999 : ia) - (ib === -1 ? 999 : ib);
    }),
);
const otherSchemas = computed(() => schemas.value.filter((s) => s.category === "other"));

watch(
  () => route.path,
  (p) => {
    const seg = p.split("/");
    if (seg[1] === "dns" && seg[2]) {
      selectedKeys.value = [`dns/${seg[2]}`];
      const s = schemas.value.find((x) => x.key === seg[2]);
      openKeys.value = s ? [s.category === "mainstream" ? "dns-mainstream" : "dns-other"] : [];
    } else {
      selectedKeys.value = [seg[1] || "certs"];
    }
  },
  { immediate: true },
);

const onMenuClick = (e: { key: string | number }) => {
  router.push(`/${e.key}`);
};

const onUserMenu = (e: { key: string | number }) => {
  if (e.key === "pwd") {
    showPwd.value = true;
  } else if (e.key === "logout") {
    void auth.logout().then(() => {
      message.success("已退出登录");
      router.push("/login");
    });
  }
};

// antd v5 风格: 菜单项数据化
const userMenuItems = [
  { key: "pwd", label: "修改密码", icon: () => h(KeyRound, { size: 14 }) },
  { key: "logout", label: "退出登录", icon: () => h(LogOut, { size: 14 }) },
];

const redetectAcme = async () => {
  try {
    meta.value = await api.meta();
    if (meta.value?.acme_sh.found) message.success("已检测到 acme.sh");
  } catch {
    // 忽略
  }
};

onMounted(async () => {
  try {
    meta.value = await api.meta();
  } catch {
    // 忽略
  }
  try {
    schemas.value = await getProviderSchemas();
  } catch {
    // 忽略
  }
});
</script>

<style lang="scss" scoped>
.main-layout {
  height: 100vh;
}

.sider {
  background: var(--ant-color-bg-container, #fff);
  border-right: 1px solid var(--ant-color-border-secondary, #f0f0f0);

  :deep(.ant-layout-sider-children) {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .brand {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 56px;
    padding: 0 16px;
    overflow: hidden;

    .brand-icon {
      flex: none;
      width: 32px;
      height: 32px;
      border-radius: 9px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      color: #fff;
      background: linear-gradient(135deg, #1677ff, #13c2c2);
    }

    .brand-name {
      font-size: 16px;
      font-weight: 600;
      color: var(--ant-color-text, #1f2937);
      white-space: nowrap;
    }
  }

  // Menu 组件根节点不继承父 scope 属性，必须用 :deep 才能命中
  :deep(.sider-menu) {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    border-inline-end: none !important;
    padding: 4px 8px;
  }
}

.menu-item-inner {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--ant-color-bg-container, #fff);
  border-bottom: 1px solid var(--ant-color-border-secondary, #f0f0f0);
  padding: 0 20px;
  height: 56px;
  line-height: 56px;

  .app-title {
    font-size: 14px;
    color: var(--ant-color-text-secondary, #4b5563);
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 16px;
  }
}

.acme-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--ant-color-text-secondary, #4b5563);
  cursor: default;

  .acme-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  &.ok .acme-dot {
    background: #52c41a;
    box-shadow: 0 0 0 3px rgba(82, 196, 26, 0.15);
  }

  &.bad .acme-dot {
    background: #ff4d4f;
    box-shadow: 0 0 0 3px rgba(255, 77, 79, 0.15);
  }

  &.clickable {
    cursor: pointer;

    &:hover {
      color: var(--ant-color-text, #1f2937);
    }
  }
}

.user-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--ant-color-text-secondary, #374151);
  cursor: pointer;
  transition: background 0.2s;

  &:hover {
    background: var(--ant-color-bg-text-hover, rgba(0, 0, 0, 0.06));
  }
}

.content {
  overflow: auto;
}
</style>
