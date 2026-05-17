import ColorBends from "@/components/ColorBends";
import LiquidEther from "@/components/LiquidEther";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ChartContainer } from "@/components/ui/chart";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { tauriApi } from "@/lib/tauri";
import type { AppSettings, AppStatus, UsageSnapshot } from "@/types/contracts";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MinusIcon, SquareIcon, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  PolarAngleAxis,
  RadialBar,
  RadialBarChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

function statusDotClasses(isConnected: boolean): {
  ping: string;
  dot: string;
} {
  if (isConnected) {
    return { ping: "bg-emerald-400", dot: "bg-emerald-500" };
  }
  return { ping: "bg-red-400", dot: "bg-red-500" };
}

function formatDateTime(value?: string): string {
  if (!value) return "Unknown";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(d);
}

function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}

function bucketLabel(label: string): string {
  if (label === "today") return "Today";
  if (label === "thisWeek") return "This Week";
  if (label === "thisMonth") return "This Month";
  return label;
}

function tokenShort(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return `${value}`;
}

const DEFAULT_SETTINGS: AppSettings = {
  refreshIntervalSeconds: 30,
  startOnLogin: false,
  notificationsEnabled: true,
  notificationThresholds: [50, 75, 90, 100],
  showFallbackWhenRpcUnavailable: true,
};

export default function App(): JSX.Element {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [snapshot, setSnapshot] = useState<UsageSnapshot | null>(null);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [uiError, setUiError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [nextStatus, nextSnapshot] = await Promise.all([
          tauriApi.getAppStatus(),
          tauriApi.getUsageSnapshot(),
        ]);
        const nextSettings = await tauriApi.getSettings();
        setStatus(nextStatus);
        setSnapshot(nextSnapshot);
        setSettings(nextSettings);
        setUiError(null);
      } catch (error) {
        setUiError(String(error));
      }
    })();
  }, []);

  useEffect(() => {
    const timer = setInterval(() => {
      void tauriApi
        .refreshUsage()
        .then(setSnapshot)
        .catch((error) => setUiError(String(error)));
    }, 30 * 1000);
    return () => clearInterval(timer);
  }, []);

  const official = snapshot?.official;
  const hasRpcWarning = (snapshot?.parserWarnings ?? []).some((warning) =>
    warning.toLowerCase().includes("rpc unavailable"),
  );
  const isConnected = (status?.rpc.available ?? false) && !hasRpcWarning;
  const statusDot = statusDotClasses(isConnected);
  const resetText = official?.resetLabel ?? formatDateTime(official?.resetsAt);
  const officialWindows = official?.windows ?? [];
  const primaryRemainingPercent = Math.round(
    officialWindows[0]?.remainingPercent ?? 0,
  );
  const isPrimaryLowRemaining = primaryRemainingPercent <= 20;
  const remainingRingColor = isPrimaryLowRemaining ? "#facc15" : "#6ec8ff";
  const bucketChartData = (snapshot?.localBuckets ?? []).map((b) => ({
    label: bucketLabel(b.label),
    totalTokens: b.totalTokens,
    sessions: b.sessionCount,
  }));

  async function updateAppSettings(next: AppSettings): Promise<void> {
    setSettings(next);
    try {
      const saved = await tauriApi.updateSettings(next);
      setSettings(saved);
      setUiError(null);
    } catch (error) {
      setUiError(String(error));
    }
  }

  async function minimizeWindow(): Promise<void> {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow(): Promise<void> {
    await getCurrentWindow().toggleMaximize();
  }

  async function closeWindow(): Promise<void> {
    await getCurrentWindow().close();
  }

  function runWindowAction(action: () => Promise<void>): void {
    void action().catch((error) => {
      setUiError(String(error));
    });
  }

  return (
    <main className="relative min-h-screen overflow-x-hidden overflow-y-auto">
      <div className="absolute inset-0 z-0 bg-[radial-gradient(circle_at_20%_10%,#1e3a8a_0%,#0f172a_38%,#020617_100%)]" />
      <div className="pointer-events-none absolute inset-0 z-[5] opacity-70">
        <LiquidEther
          className="absolute inset-0"
          autoDemo
          autoSpeed={0.4}
          autoIntensity={1.5}
          colors={["#2563eb", "#06b6d4", "#38bdf8"]}
        />
      </div>
      <div className="pointer-events-none absolute inset-0 z-[7] opacity-55">
        <ColorBends
          className="absolute inset-0"
          colors={["#60a5fa", "#22d3ee", "#0ea5e9"]}
          rotation={90}
          speed={0.2}
          scale={1}
          frequency={1}
          warpStrength={1}
          mouseInfluence={1}
          noise={0.15}
          parallax={0.5}
          iterations={1}
          intensity={1.5}
          bandWidth={6}
          transparent
          autoRotate={0}
        />
      </div>
      <div className="absolute inset-0 z-10 bg-slate-950/35" />
      <div className="relative z-20 mx-auto max-w-6xl p-3 sm:p-4 md:p-6">
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-4 rounded-xl border border-white/15 bg-transparent p-3 shadow-2xl backdrop-blur-xl"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div data-tauri-drag-region className="pr-3">
              <h1 className="text-xl font-semibold text-slate-50">
                Codex Usage Monitor
              </h1>
            </div>
            <div
              data-tauri-drag-region
              className="h-8 min-w-12 flex-1 rounded-md border border-white/10 bg-white/5"
            />
            <div className="flex items-center gap-2">
              <div className="relative flex h-3 w-3">
                <span
                  className={`absolute inline-flex h-full w-full animate-ping rounded-full opacity-75 ${statusDot.ping}`}
                />
                <span
                  className={`relative inline-flex h-3 w-3 rounded-full ${statusDot.dot}`}
                />
              </div>
              <Sheet>
                <SheetTrigger
                  render={
                    <Button
                      variant="outline"
                      size="sm"
                      className="border-white/25 bg-white/10 text-slate-100 shadow-[0_8px_30px_rgba(14,165,233,0.2)] backdrop-blur-md transition-all hover:bg-white/20"
                    />
                  }
                >
                  Settings
                </SheetTrigger>
                <SheetContent
                  side="right"
                  className="border-white/15 bg-slate-950/95 text-slate-100 backdrop-blur-xl"
                >
                  <SheetHeader>
                    <SheetTitle className="text-slate-50">Settings</SheetTitle>
                    <SheetDescription className="text-slate-300">
                      Path overrides and system information.
                    </SheetDescription>
                  </SheetHeader>
                  <div className="space-y-4 px-4 pb-4 text-sm text-slate-100">
                    <div className="space-y-2">
                      <Label
                        htmlFor="codex-home-override"
                        className="text-slate-200"
                      >
                        Codex Home Override
                      </Label>
                      <Input
                        id="codex-home-override"
                        className="border-white/20 bg-slate-900/70 text-slate-100 placeholder:text-slate-400"
                        value={settings.codexHomeOverride ?? ""}
                        onChange={(event) => {
                          setSettings({
                            ...settings,
                            codexHomeOverride: event.target.value,
                          });
                        }}
                        onBlur={() => {
                          void updateAppSettings(settings);
                        }}
                        placeholder="~/.codex"
                      />
                    </div>
                    {uiError ? (
                      <p className="text-red-300">UI error: {uiError}</p>
                    ) : null}
                    {(snapshot?.parserWarnings ?? []).map((warning) => (
                      <p key={warning} className="text-amber-300">
                        Parser warning: {warning}
                      </p>
                    ))}
                  </div>
                </SheetContent>
              </Sheet>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 border border-white/15 bg-white/10 text-slate-100 hover:bg-white/20"
                onClick={() => runWindowAction(minimizeWindow)}
              >
                <MinusIcon className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 border border-white/15 bg-white/10 text-slate-100 hover:bg-white/20"
                onClick={() => runWindowAction(toggleMaximizeWindow)}
              >
                <SquareIcon className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 border border-red-300/30 bg-red-500/20 text-red-100 hover:bg-red-500/35"
                onClick={() => runWindowAction(closeWindow)}
              >
                <XIcon className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </motion.div>
        <div className="grid grid-cols-1 gap-3 md:gap-4 lg:grid-cols-3">
          <Card className="border-white/15 bg-slate-950/55 text-slate-50 backdrop-blur-xl lg:col-span-2">
            <CardHeader>
              <CardTitle>Official Usage Limits</CardTitle>
            </CardHeader>
            <CardContent className="grid grid-cols-1 gap-3 xl:grid-cols-2">
              {officialWindows.length > 0 ? (
                officialWindows.map((window) => (
                  <motion.div
                    key={`${window.label}-${window.resetsAt ?? "none"}`}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className="rounded-lg border border-white/15 bg-slate-900/60 p-3"
                  >
                    <p className="text-sm font-medium text-slate-100">
                      {window.label}
                    </p>
                    <div className="mt-2 flex items-center justify-between">
                      <p className="text-2xl font-bold text-slate-50">
                        {formatPercent(window.remainingPercent)}
                      </p>
                      <p className="text-xs text-slate-300">available</p>
                    </div>
                    <Progress
                      className="mt-2 bg-slate-800"
                      value={window.remainingPercent}
                      indicatorClassName={
                        window.remainingPercent <= 20
                          ? "bg-amber-400"
                          : "bg-cyan-400"
                      }
                    />
                    <p className="mt-2 text-xs text-slate-300">
                      Resets {formatDateTime(window.resetsAt)}
                    </p>
                  </motion.div>
                ))
              ) : (
                <div className="space-y-2">
                  <AnimatePresence mode="wait">
                    <motion.div
                      key={official?.usedPercent ?? 0}
                      initial={{ y: 6, opacity: 0 }}
                      animate={{ y: 0, opacity: 1 }}
                      exit={{ y: -6, opacity: 0 }}
                      className="text-3xl font-bold text-slate-50"
                    >
                      {official?.usedPercent ?? 0}%
                    </motion.div>
                  </AnimatePresence>
                  <p className="text-sm text-slate-300">Resets: {resetText}</p>
                </div>
              )}
            </CardContent>
          </Card>
          <Card className="border-white/15 bg-slate-950/55 text-slate-50 backdrop-blur-xl lg:min-h-[320px]">
            <CardHeader>
              <CardTitle>Current Availability</CardTitle>
            </CardHeader>
            <CardContent>
              <ChartContainer
                config={{
                  remaining: { label: "Remaining", color: remainingRingColor },
                }}
                className="h-[170px] sm:h-[180px] md:h-[190px] lg:h-[170px]"
              >
                <div className="relative h-full w-full">
                  <ResponsiveContainer width="100%" height="100%">
                    <RadialBarChart
                      data={[
                        {
                          name: "remaining",
                          value: primaryRemainingPercent,
                          fill: "var(--color-remaining)",
                        },
                      ]}
                      innerRadius="58%"
                      outerRadius="96%"
                      startAngle={90}
                      endAngle={-270}
                    >
                      <PolarAngleAxis
                        type="number"
                        domain={[0, 100]}
                        tick={false}
                      />
                      <RadialBar
                        dataKey="value"
                        cornerRadius={10}
                        background={{ fill: "rgba(148, 163, 184, 0.22)" }}
                      />
                      <Tooltip
                        formatter={(value) => [`${value}%`, "Available"]}
                        cursor={false}
                      />
                    </RadialBarChart>
                  </ResponsiveContainer>
                </div>
              </ChartContainer>
              <p className="mt-2 text-center text-xs text-slate-300">
                {officialWindows[0]?.label ?? "No official window detected"}
              </p>
            </CardContent>
          </Card>
        </div>
        <div className="mt-3 grid grid-cols-1 gap-3 md:gap-4">
          <Card className="border-white/15 bg-slate-950/55 text-slate-50 backdrop-blur-xl">
            <CardHeader>
              <CardTitle>Local Usage Buckets</CardTitle>
            </CardHeader>
            <CardContent>
              <ChartContainer
                config={{
                  totalTokens: {
                    label: "Total Tokens",
                    color: "#6ec8ff",
                  },
                }}
              >
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={bucketChartData}>
                    <CartesianGrid
                      vertical={false}
                      stroke="rgba(255,255,255,0.12)"
                    />
                    <XAxis
                      dataKey="label"
                      axisLine={false}
                      tickLine={false}
                      stroke="#cdd6e3"
                    />
                    <YAxis
                      tickFormatter={(value) => tokenShort(value as number)}
                      axisLine={false}
                      tickLine={false}
                      stroke="#cdd6e3"
                    />
                    <Tooltip
                      formatter={(value) => [
                        `${(value as number).toLocaleString()} tokens`,
                        "Total",
                      ]}
                      cursor={{ fill: "rgba(148, 163, 184, 0.16)" }}
                      contentStyle={{
                        background: "rgba(2, 6, 23, 0.92)",
                        border: "1px solid rgba(148, 163, 184, 0.35)",
                        borderRadius: "10px",
                        color: "#e2e8f0",
                        boxShadow: "0 12px 28px rgba(2, 6, 23, 0.5)",
                      }}
                      labelStyle={{ color: "#bae6fd", fontWeight: 600 }}
                      itemStyle={{ color: "#e2e8f0" }}
                    />
                    <Bar
                      dataKey="totalTokens"
                      fill="var(--color-totalTokens)"
                      radius={6}
                    />
                  </BarChart>
                </ResponsiveContainer>
              </ChartContainer>
            </CardContent>
          </Card>
        </div>
      </div>
    </main>
  );
}
