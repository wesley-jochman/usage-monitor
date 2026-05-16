export type PlatformName = "macos" | "windows" | "linux";

export type CodexCliStatus = {
  installed: boolean;
  path?: string;
  version?: string;
  error?: string;
};

export type CodexRpcStatus = {
  available: boolean;
  appServerPath?: string;
  lastSuccessAt?: string;
  error?: string;
};

export type CodexPathStatus = {
  codexHome: string;
  exists: boolean;
  sessionsDirExists: boolean;
  stateDbPath?: string;
  logsDbPath?: string;
};

export type AppStatus = {
  platform: PlatformName;
  cli: CodexCliStatus;
  rpc: CodexRpcStatus;
  paths: CodexPathStatus;
};

export type TokenUsage = {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type OfficialUsage = {
  available: boolean;
  limitKind?: "session" | "weekly" | "unknown";
  usedPercent?: number;
  remainingPercent?: number;
  windowMinutes?: number;
  resetsAt?: string;
  resetLabel?: string;
  planType?: string;
  rateLimitReachedType?: string;
  windows?: Array<{
    label: string;
    usedPercent: number;
    remainingPercent: number;
    windowMinutes?: number;
    resetsAt?: string;
  }>;
  status: "ok" | "warning" | "critical" | "exhausted" | "unknown";
};

export type LocalUsageBucket = {
  label: "today" | "thisWeek" | "thisMonth";
  start: string;
  end: string;
  totalTokens: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  sessionCount: number;
  budgetTokens?: number;
  budgetUsedPercent?: number;
};

export type RecentSession = {
  id: string;
  title?: string;
  cwd?: string;
  model?: string;
  startedAt: string;
  updatedAt: string;
  totalTokens: number;
};

export type UsageSnapshot = {
  official: OfficialUsage;
  localBuckets: LocalUsageBucket[];
  recentSessions: RecentSession[];
  lastUpdatedAt: string;
  parserWarnings: string[];
};

export type AppSettings = {
  codexHomeOverride?: string;
  refreshIntervalSeconds: number;
  startOnLogin: boolean;
  notificationsEnabled: boolean;
  notificationThresholds: Array<50 | 75 | 90 | 100>;
  dailyBudgetTokens?: number;
  weeklyBudgetTokens?: number;
  monthlyBudgetTokens?: number;
  showFallbackWhenRpcUnavailable: boolean;
};
