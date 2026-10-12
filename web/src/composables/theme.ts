import { computed, ref } from "vue";
import { theme as antdTheme, type ThemeConfig } from "antdv-next";

export type ThemeMode = "light" | "dark";

const MODE_KEY = "acme-webui:theme-mode";

function initialMode(): ThemeMode {
  try {
    const raw = localStorage.getItem(MODE_KEY);
    if (raw === "dark" || raw === "light") return raw;
  } catch {
    // 忽略
  }
  return "light";
}

// 模块级单一状态：App.vue（ConfigProvider）与头部切换按钮共享
const mode = ref<ThemeMode>(initialMode());

const themeConfig = computed<ThemeConfig>(() => ({
  algorithm: mode.value === "dark" ? [antdTheme.darkAlgorithm] : [antdTheme.defaultAlgorithm],
}));

// 需暴露到全局（body、登录页等非 antd 组件区域）的内置 token
const ROOT_TOKENS = [
  "colorBgLayout",
  "colorBgContainer",
  "colorBgElevated",
  "colorBorder",
  "colorBorderSecondary",
  "colorText",
  "colorTextSecondary",
  "colorTextTertiary",
  "colorTextQuaternary",
  "colorBgTextHover",
  "colorPrimary",
] as const;

const toVarName = (token: string) => "--ant-" + token.replace(/([A-Z])/g, "-$1").toLowerCase();

// antd 的 --ant-* 变量默认只定义在带 css-var 类的作用域内；
// 用内置 getDesignToken 计算当前主题 token 并同步到 <html>，
// 使全局自定义样式也能引用内置变量并跟随主题切换
function syncRootTokens() {
  const tokens = antdTheme.getDesignToken(themeConfig.value);
  const root = document.documentElement;
  for (const key of ROOT_TOKENS) {
    root.style.setProperty(toVarName(key), String(tokens[key as keyof typeof tokens]));
  }
  // dark 类仅供 color-scheme（原生滚动条/表单控件）使用
  root.classList.toggle("dark", mode.value === "dark");
}
syncRootTokens();

function setMode(m: ThemeMode) {
  mode.value = m;
  syncRootTokens();
  try {
    localStorage.setItem(MODE_KEY, m);
  } catch {
    // 忽略
  }
}

function toggle() {
  setMode(mode.value === "dark" ? "light" : "dark");
}

export function useTheme() {
  return { mode, themeConfig, setMode, toggle };
}
