import type { Checkpoint, ForecastResponse } from "../../api/types";
import {
  formatTemp,
  formatWind,
  formatPrecip,
  formatPercent,
  formatTimeWithZone,
  formatDate,
  formatCheckBackMessage,
  windDirectionLabel,
} from "../../utils/formatting";
import MiniTimeline from "./MiniTimeline";

interface CheckpointDetailProps {
  /** The selected checkpoint. */
  checkpoint: Checkpoint;
  /** ISO 8601 pass-through time. */
  passTime: string;
  /** Forecast data (null if not yet loaded). */
  forecast: ForecastResponse | null;
  /** Whether the forecast is still loading. */
  isLoading: boolean;
}

/**
 * Renders the detailed weather view for a single checkpoint.
 * Matches the wireframe from specs.md Section 5.4.
 */
export default function CheckpointDetail({
  checkpoint,
  passTime,
  forecast,
  isLoading,
}: CheckpointDetailProps) {
  if (isLoading) {
    return (
      <div className="space-y-4 p-4" aria-busy="true">
        <CheckpointHeader checkpoint={checkpoint} passTime={passTime} />
        <div className="space-y-3" role="status" aria-label="Loading forecast data">
          {[...Array(5)].map((_, i) => (
            <div
              key={i}
              className="h-12 animate-pulse rounded-lg bg-surface-alt"
            />
          ))}
        </div>
      </div>
    );
  }

  if (!forecast) {
    return (
      <div className="p-4">
        <CheckpointHeader checkpoint={checkpoint} passTime={passTime} />
        <p className="mt-4 text-text-muted">
          No forecast data available for this time.
        </p>
      </div>
    );
  }

  if (!forecast.forecast_available || forecast.weather === null) {
    const checkBackMsg = formatCheckBackMessage(forecast.forecast_horizon, passTime);

    return (
      <div className="space-y-4 p-4">
        <CheckpointHeader checkpoint={checkpoint} passTime={passTime} />
        <div className="rounded-lg bg-surface-alt p-4">
          <p className="text-sm text-text-muted">
            Forecast not yet available — the race date is beyond yr.no&apos;s
            current forecast horizon{forecast.forecast_horizon && (
              <> (<span className="text-text-secondary">{formatDate(forecast.forecast_horizon)}</span>)</>
            )}.
            {" "}{checkBackMsg}
          </p>
        </div>
      </div>
    );
  }

  const w = forecast.weather;

  return (
    <div className="space-y-4 p-4">
      <CheckpointHeader checkpoint={checkpoint} passTime={passTime} />

      {/* Stale data badge */}
      {forecast.stale && (
        <div role="alert" className="rounded-md bg-accent-warm/10 px-3 py-2 text-sm text-accent-warm">
          Forecast data may be outdated (yr.no unavailable)
        </div>
      )}

      {/* Temperature */}
      <WeatherRow label="Temperature">
        <div className="text-lg font-semibold text-text-primary">
          {formatTemp(w.temperature_c)}
          <span className="ml-2 text-sm font-normal text-text-secondary">
            (feels like {formatTemp(w.feels_like_c)})
          </span>
        </div>
        {w.temperature_percentile_10_c != null &&
          w.temperature_percentile_90_c != null && (
            <div className="text-xs text-text-muted">
              p10/p90: {formatTemp(w.temperature_percentile_10_c)} to{" "}
              {formatTemp(w.temperature_percentile_90_c)}
            </div>
          )}
      </WeatherRow>

      {/* Wind */}
      <WeatherRow label="Wind">
        <div className="text-lg font-semibold text-text-primary">
          {formatWind(w.wind_speed_ms)}{" "}
          <span className="text-accent-cool">
            {windDirectionLabel(w.wind_direction_deg)}
          </span>
          {w.wind_gust_ms != null && (
            <span className="ml-2 text-sm font-normal text-text-secondary">
              (gust {formatWind(w.wind_gust_ms)})
            </span>
          )}
        </div>
        {w.wind_speed_percentile_10_ms != null &&
          w.wind_speed_percentile_90_ms != null && (
            <div className="text-xs text-text-muted">
              p10/p90: {formatWind(w.wind_speed_percentile_10_ms)} &ndash;{" "}
              {formatWind(w.wind_speed_percentile_90_ms)}
            </div>
          )}
      </WeatherRow>

      {/* Precipitation */}
      <WeatherRow label="Precipitation">
        <div className="text-lg font-semibold text-text-primary">
          <span className="capitalize">{w.precipitation_type}</span>{" "}
          {formatPrecip(w.precipitation_mm)}
        </div>
        {w.precipitation_min_mm != null && w.precipitation_max_mm != null && (
          <div className="text-xs text-text-muted">
            range: {formatPrecip(w.precipitation_min_mm)} &ndash;{" "}
            {formatPrecip(w.precipitation_max_mm)}
          </div>
        )}
      </WeatherRow>

      {/* Humidity & Cloud cover & Dew point */}
      {w.humidity_pct != null && (
        <WeatherRow label="Humidity">
          <div className="text-text-primary">
            {formatPercent(w.humidity_pct)}
          </div>
        </WeatherRow>
      )}

      {w.dew_point_c != null && (
        <WeatherRow label="Dew point">
          <div className="text-text-primary">
            {formatTemp(w.dew_point_c)}
          </div>
        </WeatherRow>
      )}

      {w.cloud_cover_pct != null && (
        <WeatherRow label="Cloud cover">
          <div className="text-text-primary">
            {formatPercent(w.cloud_cover_pct)}
          </div>
        </WeatherRow>
      )}

      {/* UV Index (if available) */}
      {w.uv_index != null && (
        <WeatherRow label="UV index">
          <div className="text-text-primary">{w.uv_index.toFixed(1)}</div>
        </WeatherRow>
      )}

      {/* Mini Timeline */}
      <MiniTimeline checkpointId={checkpoint.id} passTime={passTime} />

      {/* Metadata */}
      <div className="border-t border-border pt-3 text-xs text-text-muted">
        {forecast.source && <div>Source: {forecast.source}</div>}
        {forecast.yr_model_run_at && (
          <div>
            Model run: {formatTimeWithZone(forecast.yr_model_run_at)}
          </div>
        )}
      </div>
    </div>
  );
}

/** Checkpoint name and expected time header. */
function CheckpointHeader({
  checkpoint,
  passTime,
}: {
  checkpoint: Checkpoint;
  passTime: string;
}) {
  return (
    <div className="border-b border-border pb-3">
      <h2 className="text-lg font-bold text-text-primary">
        {checkpoint.name}
        <span className="ml-2 text-sm font-normal text-text-secondary">
          ({checkpoint.distance_km.toFixed(1)} km)
        </span>
      </h2>
      <div className="text-sm text-text-secondary">
        Expected: {formatTimeWithZone(passTime)}
      </div>
    </div>
  );
}

/** A labeled row in the weather detail view. */
function WeatherRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-lg bg-surface-alt p-3" role="group" aria-label={label}>
      <div className="mb-1 text-xs font-medium uppercase tracking-wider text-text-muted">
        {label}
      </div>
      {children}
    </div>
  );
}
