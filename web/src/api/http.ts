import axios, { type AxiosRequestConfig } from "axios";
import { message } from "antdv-next";

export interface Envelope<T> {
  code: number;
  msg: string;
  data: T;
}

const http = axios.create({
  baseURL: "/api",
  timeout: 30000,
});

// 请求: 注入 sa-token 风格的会话头
http.interceptors.request.use((config) => {
  const token = localStorage.getItem("acme_token");
  if (token) {
    config.headers["satoken"] = token;
  }
  return config;
});

// 响应: 统一解包 + 错误提示 + 401 跳转登录
http.interceptors.response.use(
  (res) => {
    const body = res.data as Envelope<unknown>;
    if (body && typeof body.code === "number" && body.code !== 0) {
      message.error(body.msg || "请求失败");
      return Promise.reject(new Error(body.msg));
    }
    return res;
  },
  (err: any) => {
    const status = err?.response?.status;
    const msg = err?.response?.data?.msg;
    if (status === 401) {
      localStorage.removeItem("acme_token");
      localStorage.removeItem("acme_username");
      if (!window.location.pathname.startsWith("/login")) {
        window.location.href = "/login";
      }
    } else {
      message.error(msg || err?.message || "网络错误");
    }
    return Promise.reject(err);
  }
);

async function request<T>(
  method: "get" | "post" | "put" | "delete",
  url: string,
  data?: unknown,
  params?: unknown
): Promise<T> {
  const cfg: AxiosRequestConfig = { method, url, data, params };
  const res = await http.request<Envelope<T>>(cfg);
  return (res.data as Envelope<T>).data;
}

export const get = <T>(url: string, params?: unknown) =>
  request<T>("get", url, undefined, params);
export const post = <T>(url: string, data?: unknown) => request<T>("post", url, data);
export const put = <T>(url: string, data?: unknown) => request<T>("put", url, data);
export const del = <T>(url: string, data?: unknown) => request<T>("delete", url, data);
