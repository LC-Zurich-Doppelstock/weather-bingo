import { memo, useCallback, useMemo } from "react";
import {
  ResponsiveContainer,
  ComposedChart,
  Line,
  Area,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
} from "recharts";
import type { CategoricalChartState } from "recharts/types/chart/types";
import type { Checkpoint, RaceForecastResponse } from "../../api/types";
import { chartColors, colors, uncertaintyOpacity } from "../../styles/theme";
import { formatTemp, formatWind, formatPrecip, windDirectionLabel, formatTimeWithZone } from "../../utils/formatting";

interface CourseOverviewProps {
  /** Race forecast data for all checkpoints. */
  raceForecast: RaceForecastResponse | null;
  /** Checkpoints (used for x-axis labels). */
  checkpoints: Checkpoint[];
  /** Whether data is loading. */
  isLoading: boolean;
  /** Currently hovered checkpoint ID (from map or chart). */
  hoveredCheckpointId: string | null;
  /** Callback when a checkpoint is hovered/unhovered on the chart. */
  onCheckpointHover: (id: string | null) => void;
}

interface ChartDataPoint {
  checkpointId: string;
  name: string;
  distance: number;
  temperature: number;
  tempP10: number | null;
  tempP90: number | null;
  tempRange: [number, number] | null;
  feelsLike: number;
  wind: number;
  windP10: number | null;
  windP90: number | null;
  windRange: [number, number] | null;
  windDirection: string;
  precipitation: number;
}

/**
 * Course overview with 3 stacked sparkline charts showing weather
 * along the entire race course (Section 5.5).
 */
const CourseOverview = memo(function CourseOverview({
  raceForecast,
  checkpoints,
  isLoading,
  hoveredCheckpointId,
  onCheckpointHover,
}: CourseOverviewProps) {
  // Whether any checkpoint in the race forecast is missing weather data
  const hasAnyUnavailable = useMemo(() => {
    if (!raceForecast) return false;
    return raceForecast.checkpoints.some((cp) => cp.weather === null);
  }, [raceForecast]);

  const allUnavailable = useMemo(() => {
    if (!raceForecast || raceForecast.checkpoints.length === 0) return false;
    return raceForecast.checkpoints.every((cp) => cp.weather === null);
  }, [raceForecast]);

  // Build chart data from race forecast — only checkpoints with weather data
  const data: ChartDataPoint[] = useMemo(() => {
    if (!raceForecast || raceForecast.checkpoints.length === 0) return [];
    return raceForecast.checkpoints
      .filter((cp) => cp.weather !== null)
      .map((cp) => {
        // Safe: we just filtered for non-null weather above
        const w = cp.weather!;
        return {
          checkpointId: cp.checkpoint_id,
          name: cp.name.split(" ")[0] ?? cp.name,
          distance: cp.distance_km,
          temperature: w.temperature_c,
          tempP10: w.temperature_percentile_10_c ?? null,
          tempP90: w.temperature_percentile_90_c ?? null,
          tempRange:
            w.temperature_percentile_10_c != null &&
            w.temperature_percentile_90_c != null
              ? [w.temperature_percentile_10_c, w.temperature_percentile_90_c] as [number, number]
              : null,
          feelsLike: w.feels_like_c,
          wind: w.wind_speed_ms,
          windP10: w.wind_speed_percentile_10_ms ?? null,
          windP90: w.wind_speed_percentile_90_ms ?? null,
          windRange:
            w.wind_speed_percentile_10_ms != null &&
            w.wind_speed_percentile_90_ms != null
              ? [w.wind_speed_percentile_10_ms, w.wind_speed_percentile_90_ms] as [number, number]
              : null,
          windDirection: windDirectionLabel(w.wind_direction_deg),
          precipitation: w.precipitation_mm,
        };
      });
  }, [raceForecast]);

  // Compute the active chart index from the hovered checkpoint ID
  const activeIndex = useMemo(() => {
    if (!hoveredCheckpointId || data.length === 0) return undefined;
    const idx = data.findIndex((d) => d.checkpointId === hoveredCheckpointId);
    return idx >= 0 ? idx : undefined;
  }, [hoveredCheckpointId, data]);

  // Distance of the hovered checkpoint (for reference line on charts)
  const hoveredDistance = activeIndex != null ? data[activeIndex]?.distance : undefined;

  // When hovering a data point on any chart, notify the parent
  const handleChartMouseMove = useCallback(
    (state: CategoricalChartState) => {
      if (state?.activeTooltipIndex != null) {
        const idx = state.activeTooltipIndex;
        const point = data[idx];
        if (point) {
          onCheckpointHover(point.checkpointId);
        }
      }
    },
    [data, onCheckpointHover]
  );

  const handleChartMouseLeave = useCallback(() => {
    onCheckpointHover(null);
  }, [onCheckpointHover]);

  if (isLoading) {
    return (
      <div className="space-y-4 p-4" aria-busy="true">
        <h2 className="text-lg font-bold text-text-primary">
          Weather Along the Course
        </h2>
        <div className="space-y-6" role="status" aria-label="Loading forecast data">
          {[...Array(3)].map((_, i) => (
            <div
              key={i}
              className="h-24 animate-pulse rounded-lg bg-surface-alt"
            />
          ))}
        </div>
      </div>
    );
  }

  if (!raceForecast) return null;

  if (data.length === 0) {
    return (
      <div className="p-4">
        <h2 className="text-lg font-bold text-text-primary">
          Weather Along the Course
        </h2>
        {allUnavailable ? (
          <div className="mt-4 rounded-lg bg-surface-alt p-4">
            <p className="text-sm text-text-muted">
              Forecast not yet available — the race date is beyond the ~10-day
              forecast horizon. Check back closer to race day.
            </p>
          </div>
        ) : (
          <p className="mt-4 text-text-muted">
            {checkpoints.length === 0
              ? "No checkpoints available"
              : "Loading forecast data..."}
          </p>
        )}
      </div>
    );
  }

  const hasTempBands = data.some((d) => d.tempP10 !== null && d.tempP90 !== null);
  const hasWindBands = data.some((d) => d.windP10 !== null && d.windP90 !== null);

  const tooltipStyle = {
    backgroundColor: colors.surface,
    border: `1px solid ${colors.border}`,
    borderRadius: "6px",
    color: colors.textPrimary,
    fontSize: "12px",
  };

  const tickStyle = { fill: colors.textMuted, fontSize: 10 };
  const axisLineStyle = { stroke: colors.border };

  return (
    <div className="space-y-1 p-4">
      <h2 className="text-lg font-bold text-text-primary">
        Weather Along the Course
      </h2>
      <p className="text-xs text-text-muted">
        {raceForecast.race_name} &middot;{" "}
        {raceForecast.target_duration_hours}h target
      </p>
      {raceForecast.yr_model_run_at && (
        <p className="text-xs text-text-muted">
          Model run: {formatTimeWithZone(raceForecast.yr_model_run_at)}
        </p>
      )}

      {hasAnyUnavailable && !allUnavailable && (
        <div className="rounded-md bg-accent-warm/10 px-3 py-2 text-xs text-accent-warm">
          Some checkpoints are beyond the forecast horizon — showing available data only.
        </div>
      )}

      {/* Temperature chart */}
      <SparklineSection title="Temperature" unit="°C">
        <ResponsiveContainer width="100%" height={140}>
          <ComposedChart
            data={data}
            margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
            onMouseMove={handleChartMouseMove}
            onMouseLeave={handleChartMouseLeave}
          >
            <XAxis
              dataKey="distance"
              tick={tickStyle}
              tickFormatter={(v: number) => `${v}`}
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
              labelFormatter={(v: number) => `${v} km`}
            />
            <ReferenceLine y={0} stroke={colors.border} strokeDasharray="3 3" />
            <ReferenceLine x={hoveredDistance ?? 0} stroke={colors.accentRose} strokeDasharray="3 3" strokeWidth={1} strokeOpacity={hoveredDistance != null ? 1 : 0} ifOverflow="hidden" />
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
              opacity={uncertaintyOpacity * 4}
              name="Feels like"
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
      </SparklineSection>

      {/* Precipitation chart */}
      <SparklineSection title="Precipitation" unit="mm">
        <ResponsiveContainer width="100%" height={110}>
          <BarChart
            data={data}
            margin={{ top: 5, right: 5, bottom: 5, left: 0 }}
            onMouseMove={handleChartMouseMove}
            onMouseLeave={handleChartMouseLeave}
          >
            <XAxis
              dataKey="distance"
              tick={tickStyle}
              tickFormatter={(v: number) => `${v}`}
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
              labelFormatter={(v: number) => `${v} km`}
            />
            <ReferenceLine x={hoveredDistance ?? 0} stroke={colors.accentRose} strokeDasharray="3 3" strokeWidth={1} strokeOpacity={hoveredDistance != null ? 1 : 0} ifOverflow="hidden" />
            <Bar
              dataKey="precipitation"
              fill={chartColors.precipitation}
              radius={[2, 2, 0, 0]}
              name="Precipitation"
            />
          </BarChart>
        </ResponsiveContainer>
      </SparklineSection>

      {/* Wind chart */}
      <SparklineSection title="Wind Speed" unit="m/s">
         <ResponsiveContainer width="100%" height={130}>
          <ComposedChart
            data={data}
            margin={{ top: 12, right: 5, bottom: 5, left: 0 }}
            onMouseMove={handleChartMouseMove}
            onMouseLeave={handleChartMouseLeave}
          >
            <XAxis
              dataKey="distance"
              tick={tickStyle}
              tickFormatter={(v: number) => `${v} km`}
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
              formatter={(value: number | [number, number], _name: string, props: { payload?: ChartDataPoint }) => {
                if (_name === "Wind p10-p90") {
                  const range = value as [number, number];
                  return [`${formatWind(range[0])} to ${formatWind(range[1])}`, "Wind p10-p90"];
                }
                const dir = props.payload?.windDirection ?? "";
                return [`${formatWind(value as number)} ${dir}`, "Wind"];
              }}
              labelFormatter={(v: number) => `${v} km`}
            />
            <ReferenceLine x={hoveredDistance ?? 0} stroke={colors.accentRose} strokeDasharray="3 3" strokeWidth={1} strokeOpacity={hoveredDistance != null ? 1 : 0} ifOverflow="hidden" />
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
              label={({ x, y, index }: { x: number; y: number; index: number }) => (
                <text
                  x={x}
                  y={y - 8}
                  textAnchor="middle"
                  fill={colors.textSecondary}
                  fontSize={9}
                >
                  {data[index]?.windDirection}
                </text>
              )}
            />
          </ComposedChart>
        </ResponsiveContainer>
      </SparklineSection>
    </div>
  );
});

export default CourseOverview;

/** Section wrapper for each sparkline chart. */
function SparklineSection({
  title,
  unit,
  children,
}: {
  title: string;
  unit: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-lg bg-surface-alt p-3" role="img" aria-label={`${title} chart`}>
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
