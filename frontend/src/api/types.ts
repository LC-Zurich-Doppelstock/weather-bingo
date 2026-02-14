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
  race_id: string;
  name: string;
  distance_km: number;
  latitude: number;
  longitude: number;
  elevation_m: number;
  sort_order: number;
}

export interface ForecastWeather {
  temperature_c: number;
  temperature_percentile_10_c: number | null;
  temperature_percentile_90_c: number | null;
  feels_like_c: number;
  wind_speed_ms: number;
  wind_speed_percentile_10_ms: number | null;
  wind_speed_percentile_90_ms: number | null;
  wind_direction_deg: number;
  wind_gust_ms: number | null;
  precipitation_mm: number;
  precipitation_min_mm: number | null;
  precipitation_max_mm: number | null;
  precipitation_type: string;
  humidity_pct: number;
  dew_point_c: number;
  cloud_cover_pct: number;
  uv_index: number | null;
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
}

export interface ForecastHistoryEntry {
  fetched_at: string; // ISO 8601
  yr_model_run_at: string | null; // ISO 8601
  weather: ForecastWeather;
}

export interface ForecastHistoryResponse {
  checkpoint_id: string;
  checkpoint_name: string;
  forecast_time: string; // ISO 8601
  history: ForecastHistoryEntry[];
}

/** Simplified weather for race-level overview. */
export interface RaceForecastCheckpoint {
  checkpoint_id: string;
  name: string;
  distance_km: number;
  expected_time: string; // ISO 8601
  forecast_available: boolean;
  weather: {
    temperature_c: number;
    temperature_percentile_10_c: number | null;
    temperature_percentile_90_c: number | null;
    feels_like_c: number;
    wind_speed_ms: number;
    wind_speed_percentile_10_ms: number | null;
    wind_speed_percentile_90_ms: number | null;
    wind_direction_deg: number;
    precipitation_mm: number;
    precipitation_type: string;
    symbol_code: string;
  } | null; // null when beyond yr.no forecast horizon
}

export interface RaceForecastResponse {
  race_id: string;
  race_name: string;
  target_duration_hours: number;
  yr_model_run_at: string | null; // ISO 8601
  checkpoints: RaceForecastCheckpoint[];
}
