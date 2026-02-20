/**
 * API client functions for the Weather Bingo REST API.
 * All endpoints are under /api/v1/.
 */

import type {
  Race,
  CoursePoint,
  Checkpoint,
  ForecastResponse,
  RaceForecastResponse,
  ForecastHistoryResponse,
} from "./types";

const BASE_URL = "/api/v1";

/**
 * Wraps a fetch response with metadata about staleness.
 * When the API returns X-Forecast-Stale: true, the data is cached
 * from a previous fetch and yr.no was unreachable for a refresh.
 */
export interface ApiResponse<T> {
  data: T;
  stale: boolean;
}

async function fetchJson<T>(url: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (err) {
    // Network failure (offline, DNS, CORS, etc.)
    if (err instanceof TypeError) {
      throw new Error(
        "Network error: unable to reach the server. Check your connection and try again."
      );
    }
    throw err;
  }
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }
  return response.json();
}

/**
 * Like fetchJson but also reads the X-Forecast-Stale header.
 * Used for forecast endpoints that may serve cached data.
 */
async function fetchJsonWithStale<T>(url: string): Promise<ApiResponse<T>> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (err) {
    if (err instanceof TypeError) {
      throw new Error(
        "Network error: unable to reach the server. Check your connection and try again."
      );
    }
    throw err;
  }
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }
  const stale = response.headers.get("X-Forecast-Stale") === "true";
  const data: T = await response.json();
  return { data, stale };
}

/** List all available races. */
export function fetchRaces(): Promise<Race[]> {
  return fetchJson<Race[]>(`${BASE_URL}/races`);
}

/** Get course coordinates for a race. */
export function fetchCourse(raceId: string): Promise<CoursePoint[]> {
  return fetchJson<CoursePoint[]>(`${BASE_URL}/races/${raceId}/course`);
}

/** Get all checkpoints for a race. */
export function fetchCheckpoints(raceId: string): Promise<Checkpoint[]> {
  return fetchJson<Checkpoint[]>(`${BASE_URL}/races/${raceId}/checkpoints`);
}

/** Get the latest forecast for a checkpoint at a specific time. */
export function fetchForecast(
  checkpointId: string,
  datetime: string
): Promise<ForecastResponse> {
  return fetchJson<ForecastResponse>(
    `${BASE_URL}/forecasts/checkpoint/${checkpointId}?datetime=${encodeURIComponent(datetime)}`
  );
}

/** Get forecasts for all checkpoints in a race (with stale header). */
export function fetchRaceForecast(
  raceId: string,
  targetDurationHours: number
): Promise<RaceForecastResponse> {
  return fetchJsonWithStale<RaceForecastResponse>(
    `${BASE_URL}/forecasts/race/${raceId}?target_duration_hours=${targetDurationHours}`
  ).then((r) => r.data);
}

/**
 * Fetch race forecast with staleness metadata.
 * Consumers that need to react to the X-Forecast-Stale header can use this.
 */
export function fetchRaceForecastWithStale(
  raceId: string,
  targetDurationHours: number
): Promise<ApiResponse<RaceForecastResponse>> {
  return fetchJsonWithStale<RaceForecastResponse>(
    `${BASE_URL}/forecasts/race/${raceId}?target_duration_hours=${targetDurationHours}`
  );
}

/** Get forecast history for a checkpoint at a specific time (one entry per model run). */
export function fetchForecastHistory(
  checkpointId: string,
  datetime: string
): Promise<ForecastHistoryResponse> {
  return fetchJson<ForecastHistoryResponse>(
    `${BASE_URL}/forecasts/checkpoint/${checkpointId}/history?datetime=${encodeURIComponent(datetime)}`
  );
}
