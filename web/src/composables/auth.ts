import { ref } from "vue";
import { api } from "../api";

const token = ref<string>(localStorage.getItem("acme_token") || "");
const username = ref<string>(localStorage.getItem("acme_username") || "");

export function useAuth() {
  const set = (t: string, u: string) => {
    token.value = t;
    username.value = u;
    localStorage.setItem("acme_token", t);
    localStorage.setItem("acme_username", u);
  };

  const clear = () => {
    token.value = "";
    username.value = "";
    localStorage.removeItem("acme_token");
    localStorage.removeItem("acme_username");
  };

  const logout = async () => {
    try {
      await api.logout();
    } catch {
      // 忽略登出接口错误
    }
    clear();
  };

  /** 校验当前 token 是否有效并刷新用户名 */
  const ensureMe = async () => {
    try {
      const me = await api.me();
      username.value = me.username;
    } catch {
      // 401 已由拦截器统一处理
    }
  };

  return { token, username, set, clear, logout, ensureMe };
}
