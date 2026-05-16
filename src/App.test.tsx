import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("recharts", async () => {
  const React = await import("react");
  const Stub = ({ children }: { children?: React.ReactNode }) =>
    React.createElement("div", null, children);
  return {
    ResponsiveContainer: Stub,
    RadialBarChart: Stub,
    RadialBar: Stub,
    PolarAngleAxis: Stub,
    Tooltip: Stub,
    BarChart: Stub,
    Bar: Stub,
    CartesianGrid: Stub,
    XAxis: Stub,
    YAxis: Stub,
  };
});

vi.mock("@/components/LiquidEther", async () => {
  const React = await import("react");
  return {
    default: () =>
      React.createElement("div", { "data-testid": "liquid-ether" }),
  };
});

vi.mock("@/components/ColorBends", async () => {
  const React = await import("react");
  return {
    default: () => React.createElement("div", { "data-testid": "lightning" }),
  };
});

vi.mock("@/lib/tauri", () => ({
  tauriApi: {
    getAppStatus: vi.fn().mockResolvedValue({
      platform: "macos",
      cli: { installed: true },
      rpc: { available: true },
      paths: { codexHome: "~/.codex", exists: true, sessionsDirExists: true },
    }),
    getUsageSnapshot: vi.fn().mockResolvedValue({
      official: {
        available: true,
        usedPercent: 55,
        status: "warning",
        resetLabel: "Soon",
      },
      localBuckets: [],
      recentSessions: [],
      lastUpdatedAt: "2026-05-16T00:00:00Z",
      parserWarnings: [],
    }),
    getSettings: vi.fn().mockResolvedValue({
      refreshIntervalSeconds: 60,
      startOnLogin: false,
      notificationsEnabled: true,
      notificationThresholds: [50, 75, 90, 100],
      showFallbackWhenRpcUnavailable: true,
    }),
    refreshUsage: vi.fn().mockResolvedValue({
      official: {
        available: true,
        usedPercent: 55,
        status: "warning",
        resetLabel: "Soon",
      },
      localBuckets: [],
      recentSessions: [],
      lastUpdatedAt: "2026-05-16T00:00:00Z",
      parserWarnings: [],
    }),
    showMainWindow: vi.fn().mockResolvedValue(undefined),
    hideMainWindow: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("App", () => {
  it("renders dashboard header", async () => {
    render(<App />);
    expect(await screen.findByText("Codex Usage Monitor")).toBeInTheDocument();
    expect(await screen.findByText("55%")).toBeInTheDocument();
  });
});
