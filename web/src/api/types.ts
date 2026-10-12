export type CertStatus = "pending" | "issuing" | "issued" | "failed";

export interface CertItem {
  id: number;
  name: string;
  provider_id: number;
  provider_type: string | null;
  provider_name: string | null;
  domains: string[];
  cert_path: string;
  key_path: string;
  ca: string;
  renewal_days: number;
  post_command: string;
  status: CertStatus;
  last_issued_at: string | null;
  cert_expire_at: string | null;
  next_renew_at: string | null;
  last_error: string;
  created_at: string;
  updated_at: string;
}

export interface CertPayload {
  name?: string;
  provider_id: number;
  domains: string[];
  cert_path: string;
  key_path: string;
  ca?: string;
  renewal_days: number;
  post_command?: string;
}

export interface ProviderItem {
  id: number;
  name: string;
  provider_type: string;
  type_label: string;
  dns_hook: string | null;
  credentials: Record<string, string>;
  enabled: boolean;
  cert_count: number;
  created_at: string;
  updated_at: string;
}

export interface ProviderSchemaField {
  key: string;
  label: string;
  placeholder: string;
  required: boolean;
  secret: boolean;
  help: string;
}

export type ProviderCategory = "mainstream" | "other";

export interface ProviderSchema {
  key: string;
  label: string;
  labelEn: string;
  dnsHook: string;
  category: ProviderCategory;
  description: string;
  fields: ProviderSchemaField[];
  requireAny: string[][];
  noCreds: boolean;
  docUrl: string;
}

export interface RecordItem {
  id: number;
  cert_id: number;
  cert_name: string;
  trigger: string;
  issued_at: string;
  expire_at: string | null;
  provider_type: string;
  provider_label: string;
  provider_name: string;
  domains: string[];
  cert_path: string;
  key_path: string;
  ca: string;
  success: boolean;
  message: string;
  has_output: boolean;
}

export interface RecordDetail extends RecordItem {
  command_output: string;
}

export interface AcmeInstallStep {
  title: string;
  commands: string[];
}

export interface AcmeInstallHint {
  os: string;
  os_name: string;
  primary_cmd: string;
  steps: AcmeInstallStep[];
  notes: string[];
}

export interface MetaInfo {
  cas: string[];
  ca_default: string;
  acme_sh: {
    found: boolean;
    path: string | null;
    install: AcmeInstallHint;
  };
}
