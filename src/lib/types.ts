export interface UsageWindow {
  label: string;
  used: number;
  limit: number | null;
  resetsAt: string | null;
}

export interface UsageSnapshot {
  providerId: string;
  providerName: string;
  accountId: string | null;
  account: string | null;
  plan: string | null;
  windows: UsageWindow[];
  error: string | null;
  needsAuth: boolean;
  credentialIssue?: "missing" | "unreadable" | "invalid" | "unsupported" | "ambiguous" | "expired" | "rejected" | "changed";
  fetchIssue?: "network" | "service" | "response";
  stale?: boolean;
  lastSuccessAt?: string;
  managed: boolean;
  loginHint: string | null;
  note: string | null;
  periodEndsAt: string | null;
  billing: Billing | null;
  billingConfigured: boolean;
  hidden: boolean;
  fetchedAt: string;
}

export interface ModelUsage {
  model: string;
  included: number;
  includedAmount: number;
  additional: number;
  additionalAmount: number;
}

export interface Billing {
  models: ModelUsage[];
  additionalAmount: number;
  error: string | null;
}

export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  deviceCode: string;
  interval: number;
}

export type TrayDisplay =
  | { mode: "lowest" }
  | { mode: "pinned"; provider: string; account: string }
  | { mode: "iconOnly" };
