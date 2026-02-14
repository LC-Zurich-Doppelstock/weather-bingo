import { memo } from "react";
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
import type { Checkpoint, RaceForecastResponse } from "../../api/types";
import { chartColors, uncertaintyOpacity } from "../../styles/theme";
import { formatTemp, formatWind, formatPrecip, windDirectionLabel, formatTimeWithZone } from "../../utils/formatting";

interface CourseOverviewProps {
  /** Race forecast data for all checkpoints. */
  raceForecast: RaceForecastResponse | null;
  /** Checkpoints (used for x-axis labels). */
  checkpoints: Checkpoint[];
  /** Whether data is loading. */
  isLoading: boolean;
  /** When the forecast data was last fetched (ISO 8601). */
  dataUpdatedAt: string | null;
}

interface ChartDataPoint {
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
  dataUpdatedAt,
}: CourseOverviewProps) {
  if (isLoading) {
    return (
      <div className="space-y-4 p-4">
        <h2 className="text-lg font-bold text-text-primary">
          Weather Along the Course
        </h2>
        <div className="space-y-6">
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

  if (!raceForecast || raceForecast.checkpoints.length === 0) {
    return (
      <div className="p-4">
        <h2 className="text-lg font-bold text-text-primary">
          Weather Along the Course
        </h2>
        <p className="mt-4 text-text-muted">
          {checkpoints.length === 0
            ? "No checkpoints available"
            : "Loading forecast data..."}
        </p>
      </div>
    );
  }

  const data: ChartDataPoint[] = raceForecast.checkpoints.map((cp) => ({
    name: cp.name.split(" ")[0] ?? cp.name, // Short name for x-axis
    distance: cp.distance_km,
    temperature: cp.weather.temperature_c,
    tempP10: cp.weather.temperature_percentile_10_c ?? null,
    tempP90: cp.weather.temperature_percentile_90_c ?? null,
    tempRange:
      cp.weather.temperature_percentile_10_c != null &&
      cp.weather.temperature_percentile_90_c != null
        ? [cp.weather.temperature_percentile_10_c, cp.weather.temperature_percentile_90_c]
        : null,
    feelsLike: cp.weather.feels_like_c,
    wind: cp.weather.wind_speed_ms,
    windP10: cp.weather.wind_speed_percentile_10_ms ?? null,
    windP90: cp.weather.wind_speed_percentile_90_ms ?? null,
    windRange:
      cp.weather.wind_speed_percentile_10_ms != null &&
      cp.weather.wind_speed_percentile_90_ms != null
        ? [cp.weather.wind_speed_percentile_10_ms, cp.weather.wind_speed_percentile_90_ms]
        : null,
    windDirection: windDirectionLabel(cp.weather.wind_direction_deg),
    precipitation: cp.weather.precipitation_mm,
  }));

  const hasTempBands = data.some((d) => d.tempP10 !== null && d.tempP90 !== null);
  const hasWindBands = data.some((d) => d.windP10 !== null && d.windP90 !== null);

  const tooltipStyle = {
    backgroundColor: "#171614",
    border: "1px solid #2C2A27",
    borderRadius: "6px",
    color: "#F0EEEB",
    fontSize: "12px",
  };

  return (
    <div className="space-y-1 p-4">
      <h2 className="text-lg font-bold text-text-primary">
        Weather Along the Course
      </h2>
      <p className="text-xs text-text-muted">
        {raceForecast.race_name} &middot;{" "}
        {raceForecast.target_duration_hours}h target
      </p>
      {dataUpdatedAt && (
        <p className="text-xs text-text-muted">
          Forecast retrieved: {formatTimeWithZone(dataUpdatedAt)}
        </p>
      )}

      {/* Temperature chart */}
      <SparklineSection title="Temperature" unit="°C">
        <ResponsiveContainer width="100%" height={140}>
          <ComposedChart data={data} margin={{ top: 5, right: 5, bottom: 5, left: 0 }}>
            <XAxis
              dataKey="distance"
              tick={{ fill: "#6B6762", fontSize: 10 }}
              tickFormatter={(v: number) => `${v}`}
              axisLine={{ stroke: "#2C2A27" }}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: "#6B6762", fontSize: 10 }}
              tickFormatter={(v: number) => formatTemp(v)}
              axisLine={false}
              tickLine={false}
              width={45}
            />
            <Tooltip
              contentStyle={tooltipStyle}
              formatter={(value: number | [number, number], name: string) => {
                if (name === "Temp p10–p90") {
                  const range = value as [number, number];
                  return [`${formatTemp(range[0])} to ${formatTemp(range[1])}`, "Temp p10–p90"];
                }
                return [formatTemp(value as number), name];
              }}
              labelFormatter={(v: number) => `${v} km`}
            />
            <ReferenceLine y={0} stroke="#2C2A27" strokeDasharray="3 3" />
            {hasTempBands && (
              <Area
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
          <BarChart data={data} margin={{ top: 5, right: 5, bottom: 5, left: 0 }}>
            <XAxis
              dataKey="distance"
              tick={{ fill: "#6B6762", fontSize: 10 }}
              tickFormatter={(v: number) => `${v}`}
              axisLine={{ stroke: "#2C2A27" }}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: "#6B6762", fontSize: 10 }}
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
          <ComposedChart data={data} margin={{ top: 12, right: 5, bottom: 5, left: 0 }}>
            <XAxis
              dataKey="distance"
              tick={{ fill: "#6B6762", fontSize: 10 }}
              tickFormatter={(v: number) => `${v} km`}
              axisLine={{ stroke: "#2C2A27" }}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: "#6B6762", fontSize: 10 }}
              tickFormatter={(v: number) => formatWind(v)}
              axisLine={false}
              tickLine={false}
              width={45}
            />
            <Tooltip
              contentStyle={tooltipStyle}
              formatter={(value: number | [number, number], _name: string, props: { payload?: ChartDataPoint }) => {
                if (_name === "Wind p10–p90") {
                  const range = value as [number, number];
                  return [`${formatWind(range[0])} to ${formatWind(range[1])}`, "Wind p10–p90"];
                }
                const dir = props.payload?.windDirection ?? "";
                return [`${formatWind(value as number)} ${dir}`, "Wind"];
              }}
              labelFormatter={(v: number) => `${v} km`}
            />
            {hasWindBands && (
              <Area
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
                  fill="#9E9A93"
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
    <div className="rounded-lg bg-surface-alt p-3">
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
