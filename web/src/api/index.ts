import { del, get, post, put } from "./http";
import type {
  CertItem,
  CertPayload,
  MetaInfo,
  ProviderItem,
  ProviderSchema,
  RecordDetail,
  RecordItem,
} from "./types";

export type {
  MetaInfo,
  CertItem,
  CertPayload,
  ProviderItem,
  ProviderSchema,
  RecordDetail,
  RecordItem,
} from "./types";

export interface PageData<T> {
  total: number;
  page: number;
  page_size: number;
  list: T[];
}

export interface ProviderPayload {
  provider_type: string;
  credentials: Record<string, string>;
  enabled?: boolean;
}

// 运营商 schema 为静态注册表，前端做会话级缓存
let schemaCache: ProviderSchema[] | null = null;
export async function getProviderSchemas(): Promise<ProviderSchema[]> {
  if (!schemaCache) {
    schemaCache = (await get<ProviderSchema[]>("/providers/schema")) || [];
  }
  return schemaCache;
}

export const api = {
  // 认证
  login: (username: string, password: string) =>
    post<{ token: string; username: string }>("/auth/login", { username, password }),
  logout: () => post<unknown>("/auth/logout"),
  me: () => get<{ username: string }>("/auth/me"),
  changePassword: (old_password: string, new_password: string) =>
    put<{ changed: boolean }>("/auth/password", { old_password, new_password }),

  // 元信息
  meta: () => get<MetaInfo>("/meta"),

  // 证书配置
  certs: () => get<CertItem[]>("/certs"),
  createCert: (p: CertPayload) => post<{ id: number }>("/certs", p),
  updateCert: (id: number, p: CertPayload) => put<{ id: number }>(`/certs/${id}`, p),
  deleteCert: (id: number) => del<unknown>(`/certs/${id}`),
  batchDeleteCerts: (ids: number[]) => post<{ deleted: number }>("/certs/batch-delete", { ids }),
  renewCert: (id: number) => post<{ started: boolean }>(`/certs/${id}/renew`),

  // 运营商配置（配置名由运营商类型决定，后端按 provider_type 幂等保存）
  providers: () => get<ProviderItem[]>("/providers"),
  saveProvider: (p: ProviderPayload) => post<{ id: number }>("/providers", p),
  updateProvider: (id: number, p: ProviderPayload) => put<unknown>(`/providers/${id}`, p),
  deleteProvider: (id: number) => del<unknown>(`/providers/${id}`),

  // 颁发记录
  records: (params: { page: number; page_size: number; cert_id?: number; success?: boolean }) =>
    get<PageData<RecordItem>>("/records", params),
  recordDetail: (id: number) => get<RecordDetail>(`/records/${id}`),
  deleteRecord: (id: number) => del<unknown>(`/records/${id}`),
  batchDeleteRecords: (ids: number[]) =>
    post<{ deleted: number }>("/records/batch-delete", { ids }),
};
