import { cn } from "@/lib/utils";
import * as React from "react";

export type ChartConfig = Record<string, { label: string; color: string }>;

const ChartContext = React.createContext<ChartConfig | null>(null);

export function ChartContainer({
  config,
  className,
  children,
}: {
  config: ChartConfig;
  className?: string;
  children: React.ReactNode;
}): JSX.Element {
  const style = React.useMemo(() => {
    const vars: Record<string, string> = {};
    for (const [key, entry] of Object.entries(config)) {
      vars[`--color-${key}`] = entry.color;
    }
    return vars as React.CSSProperties;
  }, [config]);

  return (
    <ChartContext.Provider value={config}>
      <div className={cn("h-[260px] w-full text-xs", className)} style={style}>
        {children}
      </div>
    </ChartContext.Provider>
  );
}

export function useChartConfig(): ChartConfig {
  const ctx = React.useContext(ChartContext);
  if (!ctx)
    throw new Error("useChartConfig must be used inside ChartContainer");
  return ctx;
}
