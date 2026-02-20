/** API type definitions matching the REST API contracts (specs.md Section 9). */

export interface Race {
  id: string;
  name: string;
  year: number;
  start_time: string; // ISO 8601
  distance_km: number;
}

/** A single coordinate point along the race course. */
export interface CoursePoint {
  lat: number;
  lon: number;
  ele: number;
}

export interface Checkpoint {
  id: string;
  name: string;
  distance_km: number;
  latitude: number;
  longitude: number;
  elevation_m: number;
  sort_order: number;
}

/** Unified weather data. Detail-only fields are optional (absent in race overview). */
export interface ForecastWeather {
  temperature_c: number;
  temperature_percentile_10_c: number | null;
  temperature_percentile_90_c: number | null;
  feels_like_c: number;
  /** Estimated snow surface temperature in Celsius (for wax selection) */
  snow_temperature_c: number;
  wind_speed_ms: number;
  wind_speed_percentile_10_ms: number | null;
  wind_speed_percentile_90_ms: number | null;
  wind_direction_deg: number;
  /** Detail view only — absent in race overview. */
  wind_gust_ms?: number | null;
  precipitation_mm: number;
  /** Minimum expected precipitation (present in both race overview and detail). */
  precipitation_min_mm?: number | null;
  /** Maximum expected precipitation (present in both race overview and detail). */
  precipitation_max_mm?: number | null;
  precipitation_type: string;
  /** Detail view only — absent in race overview. */
  humidity_pct?: number;
  /** Detail view only — absent in race overview. */
  dew_point_c?: number;
  /** Detail view only — absent in race overview. */
  cloud_cover_pct?: number;
  /** Detail view only — absent in race overview. */
  uv_index?: number | null;
  symbol_code: string;
}

export interface ForecastResponse {
  checkpoint_id: string;
  checkpoint_name: string;
  forecast_time: string; // ISO 8601
  forecast_available: boolean;
  fetched_at: string | null; // ISO 8601, null when forecast unavailable
  yr_model_run_at: string | null; // ISO 8601
  source: string | null; // null when forecast unavailable
  stale: boolean;
  weather: ForecastWeather | null; // null when beyond yr.no forecast horizon
  forecast_horizon: string | null; // ISO 8601 — furthest timestamp in yr.no data
}

/** Simplified weather for race-level overview. Uses the unified ForecastWeather type
 *  (detail-only fields will be absent). */
export interface RaceForecastCheckpoint {
  checkpoint_id: string;
  name: string;
  distance_km: number;
  expected_time: string; // ISO 8601
  forecast_available: boolean;
  weather: ForecastWeather | null; // null when beyond yr.no forecast horizon
}

export interface RaceForecastResponse {
  race_id: string;
  race_name: string;
  target_duration_hours: number;
  yr_model_run_at: string | null; // ISO 8601
  forecast_horizon: string | null; // ISO 8601 — min horizon across all checkpoints
  checkpoints: RaceForecastCheckpoint[];
}

/* ------------------------------------------------------------------ */
/*  Forecast history (Section 9.5)                                     */
/* ------------------------------------------------------------------ */

/** A single historical forecast entry showing weather at a previous model run. */
export interface ForecastHistoryEntry {
  /** When this version of the forecast was fetched (ISO 8601). */
  fetched_at: string;
  /** When yr.no's weather model generated this forecast (ISO 8601). Null for pre-poller rows. */
  yr_model_run_at: string | null;
  /** Effective model run time: yr_model_run_at if available, otherwise fetched_at. Always populated. */
  model_run_at: string;
  /** Weather data at this model run. */
  weather: ForecastWeather;
}

/** Response from GET /api/v1/forecasts/checkpoint/:id/history?datetime=ISO8601. */
export interface ForecastHistoryResponse {
  checkpoint_id: string;
  checkpoint_name: string;
  forecast_time: string; // ISO 8601
  history: ForecastHistoryEntry[];
}
