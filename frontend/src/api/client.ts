/**
 * API client functions for the Weather Bingo REST API.
 * All endpoints are under /api/v1/.
 */

import type {
  Race,
  RaceDetail,
  Checkpoint,
  ForecastResponse,
  ForecastHistoryResponse,
  RaceForecastResponse,
} from "./types";

const BASE_URL = "/api/v1";

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }
  return response.json();
}

/** List all available races. */
export function fetchRaces(): Promise<Race[]> {
  return fetchJson<Race[]>(`${BASE_URL}/races`);
}

/** Get a single race with GPX course data. */
export function fetchRace(id: string): Promise<RaceDetail> {
  return fetchJson<RaceDetail>(`${BASE_URL}/races/${id}`);
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

/** Get forecast history for a checkpoint at a specific time. */
export function fetchForecastHistory(
  checkpointId: string,
  datetime: string
): Promise<ForecastHistoryResponse> {
  return fetchJson<ForecastHistoryResponse>(
    `${BASE_URL}/forecasts/checkpoint/${checkpointId}/history?datetime=${encodeURIComponent(datetime)}`
  );
}

/** Get forecasts for all checkpoints in a race. */
export function fetchRaceForecast(
  raceId: string,
  targetDurationHours: number
): Promise<RaceForecastResponse> {
  return fetchJson<RaceForecastResponse>(
    `${BASE_URL}/forecasts/race/${raceId}?target_duration_hours=${targetDurationHours}`
  );
}
