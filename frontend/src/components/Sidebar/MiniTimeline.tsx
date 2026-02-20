import { useMemo, useState } from "react";
import {
  ResponsiveContainer,
  ComposedChart,
  Line,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
} from "recharts";
import { useQueries } from "@tanstack/react-query";
import { fetchForecast } from "../../api/client";
import { chartColors, colors, uncertaintyOpacity } from "../../styles/theme";
import { tooltipStyle, tickStyle, axisLineStyle } from "../../styles/chartStyles";
import { formatTemp, formatPrecip, formatTime, formatWind, windDirectionLabel } from "../../utils/formatting";

const tempDotStyle = { fill: chartColors.temperature, r: 3 } as const;
const tempActiveDot = { r: 5 } as const;
const windDotStyle = { fill: chartColors.wind, r: 2 } as const;
const windActiveDot = { r: 4 } as const;

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

interface MiniTimelineProps {
  /** Checkpoint ID to show timeline for. */
  checkpointId: string;
  /** The expected pass-through time (ISO 8601). */
  passTime: string;
}

/**
 * Generate hourly time slots around the pass-through time.
 * Returns 5 slots: -2h, -1h, passTime, +1h, +2h.
 */
function generateTimeSlots(passTime: string): string[] {
  const center = new Date(passTime).getTime();
  const hourMs = 60 * 60 * 1000;
  return [-2, -1, 0, 1, 2].map((offset) =>
    new Date(center + offset * hourMs).toISOString()
  );
}

interface TimelineDataPoint {
  time: string;
  timeLabel: string;
  temperature: number | null;
  tempP10: number | null;
  tempP90: number | null;
  tempRange: [number, number] | null;
  precipitation: number | null;
  windSpeed: number | null;
  windP10: number | null;
  windP90: number | null;
  windRange: [number, number] | null;
  windDirection: string | null;
}

/**
 * Collapsible mini timeline chart showing temperature, wind, and precipitation
 * ~2 hours before and after the expected pass-through time.
 * Lazy-loads data on first expand.
 * Displayed in the checkpoint detail view (Section 5.4).
 */
export default function MiniTimeline({
  checkpointId,
  passTime,
}: MiniTimelineProps) {
  const [expanded, setExpanded] = useState(false);
  const timeSlots = generateTimeSlots(passTime);

  // Lazy: only fetch when expanded
  const queries = useQueries({
    queries: timeSlots.map((datetime) => ({
      queryKey: ["forecast", checkpointId, datetime],
      queryFn: () => fetchForecast(checkpointId, datetime),
      enabled: expanded && !!checkpointId,
      staleTime: 60_000, // 1 minute
    })),
  });

  const isLoading = expanded && queries.some((q) => q.isLoading);
  const hasData = queries.some((q) => q.data);

  const data: TimelineDataPoint[] = useMemo(() => {
    if (!expanded) return [];
    return timeSlots.map((slot, i) => {
      const forecast = queries[i]?.data;
      const w = forecast?.weather;
      return {
        time: slot,
        timeLabel: formatTime(slot),
        temperature: w?.temperature_c ?? null,
        tempP10: w?.temperature_percentile_10_c ?? null,
        tempP90: w?.temperature_percentile_90_c ?? null,
        tempRange:
          w?.temperature_percentile_10_c != null && w?.temperature_percentile_90_c != null
            ? [w.temperature_percentile_10_c, w.temperature_percentile_90_c]
            : null,
        precipitation: w?.precipitation_mm ?? null,
        windSpeed: w?.wind_speed_ms ?? null,
        windP10: w?.wind_speed_percentile_10_ms ?? null,
        windP90: w?.wind_speed_percentile_90_ms ?? null,
        windRange:
          w?.wind_speed_percentile_10_ms != null && w?.wind_speed_percentile_90_ms != null
            ? [w.wind_speed_percentile_10_ms, w.wind_speed_percentile_90_ms]
            : null,
        windDirection: w?.wind_direction_deg != null
          ? windDirectionLabel(w.wind_direction_deg)
          : null,
      };
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expanded, passTime, queries.map((q) => q.data).join(",")]);

  const hasTemperatureData = data.some((d) => d.temperature !== null);
  const hasTempBands = data.some((d) => d.tempP10 !== null && d.tempP90 !== null);
  const hasWindBands = data.some((d) => d.windP10 !== null && d.windP90 !== null);

  return (
    <div className="border-t border-border pt-3">
      {/* Collapsible header */}
      <button
        className="flex w-full items-center justify-between text-left"
        onClick={() => setExpanded((prev) => !prev)}
        aria-expanded={expanded}
        aria-controls="mini-timeline-panel"
        aria-label="When Paced Differently"
      >
        <span className="text-xs font-medium uppercase tracking-wider text-text-muted">
          When Paced Differently
        </span>
        <ChevronIcon collapsed={!expanded} />
      </button>

      {/* Collapsible body */}
      <div
        id="mini-timeline-panel"
        className={`overflow-hidden transition-all duration-200 ${
          expanded ? "max-h-[400px]" : "max-h-0"
        }`}
      >
        <div className="mt-3">
          {isLoading && !hasData && (
            <div className="rounded-lg bg-surface-alt p-3">
              <div className="h-20 animate-pulse rounded bg-background" />
            </div>
          )}

          {!isLoading && expanded && !hasTemperatureData && hasData && (
            <p className="text-sm text-text-muted">
              No timeline data available.
            </p>
          )}

          {hasTemperatureData && (
            <div className="rounded-lg bg-surface-alt p-3" role="img" aria-label="Weather at different paces chart">
              <ResponsiveContainer width="100%" height={160}>
                <ComposedChart
                  data={data}
                  margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
                >
                  <XAxis
                    dataKey="timeLabel"
                    tick={tickStyle}
                    axisLine={axisLineStyle}
                    tickLine={false}
                  />
                  <YAxis
                    yAxisId="temp"
                    tick={tickStyle}
                    tickFormatter={(v: number) => formatTemp(v)}
                    axisLine={false}
                    tickLine={false}
                    width={40}
                  />
                  <YAxis
                    yAxisId="precip"
                    orientation="right"
                    tick={tickStyle}
                    tickFormatter={(v: number) => formatPrecip(v)}
                    axisLine={false}
                    tickLine={false}
                    width={40}
                    hide
                  />
                  <YAxis
                    yAxisId="wind"
                    orientation="right"
                    tick={tickStyle}
                    tickFormatter={(v: number) => formatWind(v)}
                    axisLine={false}
                    tickLine={false}
                    width={40}
                    hide
                  />
                  <Tooltip
                    contentStyle={tooltipStyle}
                    formatter={(value: number | [number, number], name: string, props: { payload?: TimelineDataPoint }) => {
                      if (name === "Temperature") return [formatTemp(value as number), name];
                      if (name === "Temp p10–p90") {
                        const range = value as [number, number];
                        return [`${formatTemp(range[0])} to ${formatTemp(range[1])}`, "Temp p10–p90"];
                      }
                      if (name === "Precipitation") return [formatPrecip(value as number), name];
                      if (name === "Wind") {
                        const dir = props.payload?.windDirection ?? "";
                        return [`${formatWind(value as number)} ${dir}`, name];
                      }
                      if (name === "Wind p10–p90") {
                        const range = value as [number, number];
                        return [`${formatWind(range[0])} to ${formatWind(range[1])}`, "Wind p10–p90"];
                      }
                      return [value, name];
                    }}
                  />
                  {/* Reference line at pass-through time */}
                  <ReferenceLine
                    x={formatTime(passTime)}
                    stroke={colors.primary}
                    strokeDasharray="3 3"
                    strokeWidth={1.5}
                    yAxisId="temp"
                  />
                  {/* Precipitation as area */}
                  <Area
                    yAxisId="precip"
                    type="monotone"
                    dataKey="precipitation"
                    fill={chartColors.precipitation}
                    fillOpacity={0.2}
                    stroke={chartColors.precipitation}
                    strokeWidth={1}
                    name="Precipitation"
                    connectNulls
                  />
                  {/* Temperature p10–p90 uncertainty band */}
                  {hasTempBands && (
                    <Area
                      yAxisId="temp"
                      type="monotone"
                      dataKey="tempRange"
                      fill={chartColors.temperature}
                      fillOpacity={uncertaintyOpacity}
                      stroke="none"
                      name="Temp p10–p90"
                      connectNulls
                      activeDot={false}
                    />
                  )}
                  {/* Temperature as line */}
                  <Line
                    yAxisId="temp"
                    type="monotone"
                    dataKey="temperature"
                    stroke={chartColors.temperature}
                    strokeWidth={2}
                    dot={tempDotStyle}
                    activeDot={tempActiveDot}
                    name="Temperature"
                    connectNulls
                  />
                  {/* Wind p10–p90 uncertainty band */}
                  {hasWindBands && (
                    <Area
                      yAxisId="wind"
                      type="monotone"
                      dataKey="windRange"
                      fill={chartColors.wind}
                      fillOpacity={uncertaintyOpacity}
                      stroke="none"
                      name="Wind p10–p90"
                      connectNulls
                      activeDot={false}
                    />
                  )}
                  {/* Wind speed as dashed line */}
                  <Line
                    yAxisId="wind"
                    type="monotone"
                    dataKey="windSpeed"
                    stroke={chartColors.wind}
                    strokeWidth={1.5}
                    strokeDasharray="4 2"
                    dot={windDotStyle}
                    activeDot={windActiveDot}
                    name="Wind"
                    connectNulls
                  />
                </ComposedChart>
              </ResponsiveContainer>
              <div className="mt-1 flex items-center justify-center gap-3 text-xs text-text-muted">
                <span>Dashed line = pass time</span>
                <span className="inline-block h-px w-3" style={{ borderTop: `1.5px dashed ${chartColors.wind}` }} />
                <span>Wind</span>
                <span>Shaded = p10–p90</span>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
