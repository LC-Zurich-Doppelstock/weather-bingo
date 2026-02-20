import { useMemo, useState } from "react";
import {
  ResponsiveContainer,
  ComposedChart,
  Line,
  Area,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
} from "recharts";
import { useForecastHistory } from "../../hooks/useForecast";
import type { ForecastHistoryEntry } from "../../api/types";
import {
  chartColors,
  uncertaintyOpacity,
  secondaryLineOpacity,
} from "../../styles/theme";
import { tooltipStyle, tickStyle, axisLineStyle } from "../../styles/chartStyles";
import { formatTemp, formatWind, formatPrecip } from "../../utils/formatting";

/** Chevron-down SVG icon (rotates when collapsed). */
function ChevronIcon({ collapsed }: { collapsed: boolean }) {
  return (
    <svg
      className={`h-4 w-4 text-text-muted transition-transform duration-200 ${
        collapsed ? "-rotate-90" : ""
      }`}
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
    </svg>
  );
}

/** Data point for history charts. */
interface HistoryDataPoint {
  /** ISO 8601 model_run_at — kept for tooltip formatting. */
  modelRunAt: string;
  /** Epoch ms of model_run_at — used as numeric x-axis dataKey. */
  modelRunEpoch: number;
  temperature: number;
  feelsLike: number;
  snowTemperature: number;
  tempRange: [number, number] | null;
  precipitation: number;
  wind: number;
  windRange: [number, number] | null;
}

/** Format ISO timestamp to short date label: "Feb 19" or "Feb 19 18:00". */
function formatModelRunLabel(iso: string, showTime: boolean): string {
  const d = new Date(iso);
  const month = d.toLocaleDateString("en-US", { month: "short", timeZone: "Europe/Stockholm" });
  const day = d.toLocaleDateString("en-US", { day: "numeric", timeZone: "Europe/Stockholm" });
  if (!showTime) return `${month} ${day}`;
  const time = d.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    timeZone: "Europe/Stockholm",
  });
  return `${month} ${day} ${time}`;
}

/** Format epoch ms to short tick label, using the showTime flag from closure. */
function makeTickFormatter(showTime: boolean) {
  return (epoch: number) =>
    formatModelRunLabel(new Date(epoch).toISOString(), showTime);
}

/** Format epoch ms for tooltip: "Feb 19, 18:00". */
function formatTooltipFromEpoch(epoch: number): string {
  return formatTooltipLabel(new Date(epoch).toISOString());
}

/** Format ISO timestamp for tooltip: "Feb 19, 18:00". */
function formatTooltipLabel(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    timeZone: "Europe/Stockholm",
  });
}

/** Check if all data points fall on the same calendar date. */
function allSameDate(entries: ForecastHistoryEntry[]): boolean {
  if (entries.length <= 1) return true;
  const first = new Date(entries[0]!.model_run_at).toLocaleDateString("en-US", {
    timeZone: "Europe/Stockholm",
  });
  return entries.every(
    (e) =>
      new Date(e.model_run_at).toLocaleDateString("en-US", {
        timeZone: "Europe/Stockholm",
      }) === first
  );
}

interface ForecastHistoryProps {
  /** Checkpoint UUID. */
  checkpointId: string;
  /** ISO 8601 pass-through time (used as the datetime query param). */
  passTime: string;
}

/**
 * Collapsible forecast history section showing how yr.no predictions
 * evolved across model runs. Three charts: temperature, precipitation, wind.
 * Lazy-loads data on first expand.
 */
export default function ForecastHistory({
  checkpointId,
  passTime,
}: ForecastHistoryProps) {
  const [expanded, setExpanded] = useState(false);
  const { data, isLoading, isError } = useForecastHistory(
    checkpointId,
    passTime,
    expanded
  );

  const history = useMemo(() => data?.history ?? [], [data]);

  // Determine if we need time labels (all same date) or date labels
  const needsTime = useMemo(() => allSameDate(history), [history]);

  const chartData: HistoryDataPoint[] = useMemo(() => {
    return history.map((entry) => {
      const w = entry.weather;
      return {
        modelRunAt: entry.model_run_at,
        modelRunEpoch: new Date(entry.model_run_at).getTime(),
        temperature: w.temperature_c,
        feelsLike: w.feels_like_c,
        snowTemperature: w.snow_temperature_c,
        tempRange:
          w.temperature_percentile_10_c != null &&
          w.temperature_percentile_90_c != null
            ? ([w.temperature_percentile_10_c, w.temperature_percentile_90_c] as [number, number])
            : null,
        precipitation: w.precipitation_mm,
        wind: w.wind_speed_ms,
        windRange:
          w.wind_speed_percentile_10_ms != null &&
          w.wind_speed_percentile_90_ms != null
            ? ([w.wind_speed_percentile_10_ms, w.wind_speed_percentile_90_ms] as [number, number])
            : null,
      };
    });
  }, [history]);

  const hasTempBands = chartData.some((d) => d.tempRange !== null);
  const hasWindBands = chartData.some((d) => d.windRange !== null);

  return (
    <div className="border-t border-border pt-3">
      {/* Collapsible header */}
      <button
        className="flex w-full items-center justify-between text-left"
        onClick={() => setExpanded((prev) => !prev)}
        aria-expanded={expanded}
        aria-controls="forecast-history-panel"
      >
        <span className="text-xs font-medium uppercase tracking-wider text-text-muted">
          Forecast History
        </span>
        <ChevronIcon collapsed={!expanded} />
      </button>

      {/* Collapsible body */}
      <div
        id="forecast-history-panel"
        className={`overflow-hidden transition-all duration-200 ${
          expanded ? "max-h-[600px]" : "max-h-0"
        }`}
      >
        <div className="mt-3 space-y-1">
          {isLoading && (
            <div className="space-y-2" role="status" aria-label="Loading history">
              {[...Array(3)].map((_, i) => (
                <div
                  key={i}
                  className="h-20 animate-pulse rounded-lg bg-surface-alt"
                />
              ))}
            </div>
          )}

          {isError && (
            <p className="text-sm text-error">
              Failed to load forecast history.
            </p>
          )}

          {!isLoading && !isError && history.length === 0 && (
            <p className="text-sm text-text-muted">
              No history data available yet.
            </p>
          )}

          {!isLoading && !isError && chartData.length > 0 && (
            <>
              {/* Temperature chart */}
              <HistorySection title="Temperature" unit="°C">
                <ResponsiveContainer width="100%" height={130}>
                  <ComposedChart
                    data={chartData}
                    margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
                  >
                    <XAxis
                      dataKey="modelRunEpoch"
                      type="number"
                      scale="time"
                      domain={["dataMin", "dataMax"]}
                      tickFormatter={makeTickFormatter(needsTime)}
                      tick={tickStyle}
                      axisLine={axisLineStyle}
                      tickLine={false}
                    />
                    <YAxis
                      tick={tickStyle}
                      tickFormatter={(v: number) => formatTemp(v)}
                      axisLine={false}
                      tickLine={false}
                      width={45}
                    />
                    <Tooltip
                      contentStyle={tooltipStyle}
                      formatter={(value: number | [number, number], name: string) => {
                        if (name === "Temp p10-p90") {
                          const range = value as [number, number];
                          return [`${formatTemp(range[0])} to ${formatTemp(range[1])}`, "Temp p10-p90"];
                        }
                        return [formatTemp(value as number), name];
                      }}
                      labelFormatter={formatTooltipFromEpoch}
                    />
                    {hasTempBands && (
                      <Area
                        type="monotone"
                        dataKey="tempRange"
                        fill={chartColors.temperature}
                        fillOpacity={uncertaintyOpacity}
                        stroke="none"
                        name="Temp p10-p90"
                        connectNulls
                        activeDot={false}
                      />
                    )}
                    <Line
                      type="monotone"
                      dataKey="feelsLike"
                      stroke={chartColors.feelsLike}
                      strokeWidth={1.5}
                      strokeDasharray="4 2"
                      dot={false}
                      opacity={secondaryLineOpacity}
                      name="Feels like"
                    />
                    <Line
                      type="monotone"
                      dataKey="snowTemperature"
                      stroke={chartColors.snowTemperature}
                      strokeWidth={1.5}
                      strokeDasharray="4 2"
                      dot={false}
                      name="Snow temp"
                    />
                    <Line
                      type="monotone"
                      dataKey="temperature"
                      stroke={chartColors.temperature}
                      strokeWidth={2}
                      dot={{ fill: chartColors.temperature, r: 3 }}
                      activeDot={{ r: 5 }}
                      name="Temperature"
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              </HistorySection>

              {/* Precipitation chart */}
              <HistorySection title="Precipitation" unit="mm">
                <ResponsiveContainer width="100%" height={100}>
                  <ComposedChart
                    data={chartData}
                    margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
                  >
                    <XAxis
                      dataKey="modelRunEpoch"
                      type="number"
                      scale="time"
                      domain={["dataMin", "dataMax"]}
                      tickFormatter={makeTickFormatter(needsTime)}
                      tick={tickStyle}
                      axisLine={axisLineStyle}
                      tickLine={false}
                    />
                    <YAxis
                      tick={tickStyle}
                      tickFormatter={(v: number) => formatPrecip(v)}
                      axisLine={false}
                      tickLine={false}
                      width={45}
                    />
                    <Tooltip
                      contentStyle={tooltipStyle}
                      formatter={(value: number) => [formatPrecip(value), ""]}
                      labelFormatter={formatTooltipFromEpoch}
                    />
                    <Bar
                      dataKey="precipitation"
                      fill={chartColors.precipitation}
                      radius={[2, 2, 0, 0]}
                      name="Precipitation"
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              </HistorySection>

              {/* Wind chart */}
              <HistorySection title="Wind Speed" unit="m/s">
                <ResponsiveContainer width="100%" height={120}>
                  <ComposedChart
                    data={chartData}
                    margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
                  >
                    <XAxis
                      dataKey="modelRunEpoch"
                      type="number"
                      scale="time"
                      domain={["dataMin", "dataMax"]}
                      tickFormatter={makeTickFormatter(needsTime)}
                      tick={tickStyle}
                      axisLine={axisLineStyle}
                      tickLine={false}
                    />
                    <YAxis
                      tick={tickStyle}
                      tickFormatter={(v: number) => formatWind(v)}
                      axisLine={false}
                      tickLine={false}
                      width={45}
                    />
                    <Tooltip
                      contentStyle={tooltipStyle}
                      formatter={(value: number | [number, number], name: string) => {
                        if (name === "Wind p10-p90") {
                          const range = value as [number, number];
                          return [`${formatWind(range[0])} to ${formatWind(range[1])}`, "Wind p10-p90"];
                        }
                        return [formatWind(value as number), "Wind"];
                      }}
                      labelFormatter={formatTooltipFromEpoch}
                    />
                    {hasWindBands && (
                      <Area
                        type="monotone"
                        dataKey="windRange"
                        fill={chartColors.wind}
                        fillOpacity={uncertaintyOpacity}
                        stroke="none"
                        name="Wind p10-p90"
                        connectNulls
                        activeDot={false}
                      />
                    )}
                    <Line
                      type="monotone"
                      dataKey="wind"
                      stroke={chartColors.wind}
                      strokeWidth={2}
                      dot={{ fill: chartColors.wind, r: 3 }}
                      activeDot={{ r: 5 }}
                      name="Wind"
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              </HistorySection>

              <p className="text-xs text-text-muted">
                {chartData.length} model run{chartData.length !== 1 ? "s" : ""}
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/** Section wrapper for each history chart (matches CourseOverview SparklineSection). */
function HistorySection({
  title,
  unit,
  children,
}: {
  title: string;
  unit: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-lg bg-surface-alt p-3" role="img" aria-label={`${title} history chart`}>
      <div className="mb-2 flex items-baseline justify-between">
        <span className="text-xs font-medium uppercase tracking-wider text-text-muted">
          {title}
        </span>
        <span className="text-xs text-text-muted">({unit})</span>
      </div>
      {children}
    </div>
  );
}
