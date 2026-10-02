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
  fetchedAt: string;
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
