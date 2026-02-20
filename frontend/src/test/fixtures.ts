/**
 * Shared test fixtures used across multiple test files.
 *
 * Centralises mock data so changes to API types only need updating in one place.
 */

import type {
  Checkpoint,
  ForecastResponse,
  Race,
  RaceForecastResponse,
} from "../api/types";

/* ------------------------------------------------------------------ */
/*  Races                                                              */
/* ------------------------------------------------------------------ */

export const mockVasaloppetRace: Race = {
  id: "race-1",
  name: "Vasaloppet",
  year: 2026,
  start_time: "2026-03-01T07:00:00Z",
  distance_km: 90,
};

export const mockTjejvasanRace: Race = {
  id: "race-2",
  name: "Tjejvasan",
  year: 2026,
  start_time: "2026-02-28T08:00:00Z",
  distance_km: 30,
};

/* ------------------------------------------------------------------ */
/*  Checkpoints                                                        */
/* ------------------------------------------------------------------ */

export const mockSalenCheckpoint: Checkpoint = {
  id: "cp-1",
  name: "Salen",
  distance_km: 0,
  latitude: 61.16,
  longitude: 13.27,
  elevation_m: 400,
  sort_order: 1,
};

export const mockMangsbodarnaCheckpoint: Checkpoint = {
  id: "cp-2",
  name: "Mangsbodarna",
  distance_km: 24,
  latitude: 61.12,
  longitude: 13.68,
  elevation_m: 520,
  sort_order: 2,
};

/** Standard two-checkpoint array used by Sidebar, CourseOverview, etc. */
export const mockCheckpoints: Checkpoint[] = [
  mockSalenCheckpoint,
  mockMangsbodarnaCheckpoint,
];

/* ------------------------------------------------------------------ */
/*  Forecast (single-checkpoint detail)                                */
/* ------------------------------------------------------------------ */

/** Full ForecastResponse with all detail-only fields populated. */
export const mockForecastResponse: ForecastResponse = {
  checkpoint_id: "cp-1",
  checkpoint_name: "Salen",
  forecast_time: "2026-03-01T07:00:00Z",
  forecast_available: true,
  fetched_at: "2026-02-28T12:00:00Z",
  yr_model_run_at: "2026-02-28T06:00:00Z",
  source: "yr.no",
  stale: false,
  forecast_horizon: "2026-03-09T12:00:00Z",
  weather: {
    temperature_c: -5,
    temperature_percentile_10_c: -8,
    temperature_percentile_90_c: -2,
    feels_like_c: -10,
    snow_temperature_c: -7.3,
    wind_speed_ms: 3.2,
    wind_speed_percentile_10_ms: 1.5,
    wind_speed_percentile_90_ms: 5.0,
    wind_direction_deg: 180,
    wind_gust_ms: 6.1,
    precipitation_mm: 0.5,
    precipitation_min_mm: 0.0,
    precipitation_max_mm: 1.2,
    precipitation_type: "snow",
    humidity_pct: 85,
    dew_point_c: -7,
    cloud_cover_pct: 90,
    uv_index: 0.5,
    symbol_code: "heavysnow",
  },
};

/**
 * Factory for creating a ForecastResponse with a specific time and temperature.
 * Useful for MiniTimeline tests that need multiple hourly forecasts.
 */
export function createMockForecast(
  time: string,
  tempC: number,
): ForecastResponse {
  return {
    checkpoint_id: "cp-1",
    checkpoint_name: "Salen",
    forecast_time: time,
    forecast_available: true,
    fetched_at: "2026-02-28T12:00:00Z",
    yr_model_run_at: "2026-02-28T06:00:00Z",
    source: "yr.no",
    stale: false,
    forecast_horizon: "2026-03-09T12:00:00Z",
    weather: {
      temperature_c: tempC,
      temperature_percentile_10_c: tempC - 3,
      temperature_percentile_90_c: tempC + 2,
      feels_like_c: tempC - 5,
      snow_temperature_c: Math.min(tempC - 1, 0),
      wind_speed_ms: 3.2,
      wind_speed_percentile_10_ms: 1.5,
      wind_speed_percentile_90_ms: 5.0,
      wind_direction_deg: 180,
      wind_gust_ms: 6.1,
      precipitation_mm: 0.5,
      precipitation_min_mm: 0.0,
      precipitation_max_mm: 1.2,
      precipitation_type: "snow",
      humidity_pct: 85,
      dew_point_c: -7,
      cloud_cover_pct: 90,
      uv_index: 0.5,
      symbol_code: "heavysnow",
    },
  };
}

/* ------------------------------------------------------------------ */
/*  Race forecast (course overview)                                    */
/* ------------------------------------------------------------------ */

export const mockRaceForecast: RaceForecastResponse = {
  race_id: "race-1",
  race_name: "Vasaloppet",
  target_duration_hours: 8,
  yr_model_run_at: "2026-02-28T06:00:00Z",
  forecast_horizon: "2026-03-09T12:00:00Z",
  checkpoints: [
    {
      checkpoint_id: "cp-1",
      name: "Salen",
      distance_km: 0,
      expected_time: "2026-03-01T07:00:00Z",
      forecast_available: true,
      weather: {
        temperature_c: -5,
        temperature_percentile_10_c: -8,
        temperature_percentile_90_c: -2,
        feels_like_c: -10,
        snow_temperature_c: -7.3,
        wind_speed_ms: 3.2,
        wind_speed_percentile_10_ms: 1.5,
        wind_speed_percentile_90_ms: 5.0,
        wind_direction_deg: 180,
        precipitation_mm: 0.5,
        precipitation_type: "snow",
        symbol_code: "heavysnow",
      },
    },
    {
      checkpoint_id: "cp-2",
      name: "Mangsbodarna",
      distance_km: 24,
      expected_time: "2026-03-01T09:08:00Z",
      forecast_available: true,
      weather: {
        temperature_c: -3,
        temperature_percentile_10_c: -6,
        temperature_percentile_90_c: -1,
        feels_like_c: -7,
        snow_temperature_c: -4.8,
        wind_speed_ms: 2.5,
        wind_speed_percentile_10_ms: 1.0,
        wind_speed_percentile_90_ms: 4.0,
        wind_direction_deg: 225,
        precipitation_mm: 0.2,
        precipitation_type: "snow",
        symbol_code: "lightsnow",
      },
    },
  ],
};
