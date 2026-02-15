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
import { formatTemp, formatPrecip, formatTime, formatWind, windDirectionLabel } from "../../utils/formatting";

// Static style objects hoisted outside the component to avoid recreation on every render
const tooltipStyle = {
  backgroundColor: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: "6px",
  color: colors.textPrimary,
  fontSize: "12px",
} as const;

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
  isPassTime: boolean;
}

/**
 * Mini timeline chart showing temperature and precipitation
 * ~2 hours before and after the expected pass-through time.
 * Displayed in the checkpoint detail view (Section 5.4).
 */
export default function MiniTimeline({
  checkpointId,
  passTime,
}: MiniTimelineProps) {
  const timeSlots = generateTimeSlots(passTime);

  // Fetch forecasts for each time slot in parallel
  const queries = useQueries({
    queries: timeSlots.map((datetime) => ({
      queryKey: ["forecast", checkpointId, datetime],
      queryFn: () => fetchForecast(checkpointId, datetime),
      enabled: !!checkpointId,
      staleTime: 60_000, // 1 minute
    })),
  });

  const isLoading = queries.some((q) => q.isLoading);
  const hasData = queries.some((q) => q.data);

  if (isLoading && !hasData) {
    return (
      <div className="rounded-lg bg-surface-alt p-3">
        <div className="mb-2 text-xs font-medium uppercase tracking-wider text-text-muted">
          Timeline
        </div>
        <div className="h-20 animate-pulse rounded bg-background" />
      </div>
    );
  }

  const data: TimelineDataPoint[] = timeSlots.map((slot, i) => {
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
      isPassTime: i === 2, // Center slot is the pass time
    };
  });

  // If no data at all, don't render
  if (!data.some((d) => d.temperature !== null)) {
    return null;
  }

  const hasTempBands = data.some((d) => d.tempP10 !== null && d.tempP90 !== null);
  const hasWindBands = data.some((d) => d.windP10 !== null && d.windP90 !== null);

  return (
    <div className="rounded-lg bg-surface-alt p-3" role="img" aria-label="Weather timeline chart">
      <div className="mb-2 text-xs font-medium uppercase tracking-wider text-text-muted">
        Timeline
      </div>
      <ResponsiveContainer width="100%" height={160}>
        <ComposedChart
          data={data}
          margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
        >
          <XAxis
            dataKey="timeLabel"
            tick={{ fill: colors.textMuted, fontSize: 10 }}
            axisLine={{ stroke: colors.border }}
            tickLine={false}
          />
          <YAxis
            yAxisId="temp"
            tick={{ fill: colors.textMuted, fontSize: 10 }}
            tickFormatter={(v: number) => formatTemp(v)}
            axisLine={false}
            tickLine={false}
            width={40}
          />
          <YAxis
            yAxisId="precip"
            orientation="right"
            tick={{ fill: colors.textMuted, fontSize: 10 }}
            tickFormatter={(v: number) => formatPrecip(v)}
            axisLine={false}
            tickLine={false}
            width={40}
            hide
          />
          <YAxis
            yAxisId="wind"
            orientation="right"
            tick={{ fill: colors.textMuted, fontSize: 10 }}
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
            dot={{ fill: chartColors.temperature, r: 3 }}
            activeDot={{ r: 5 }}
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
            dot={{ fill: chartColors.wind, r: 2 }}
            activeDot={{ r: 4 }}
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
  );
}
