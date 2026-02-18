import { useQuery } from "@tanstack/react-query";
import {
  fetchForecast,
  fetchRaceForecastWithStale,
} from "../api/client";
import type { RaceForecastResponse } from "../api/types";

/** Fetch the latest forecast for a checkpoint at a specific time. */
export function useForecast(checkpointId: string | null, datetime: string | null) {
  return useQuery({
    queryKey: ["forecast", checkpointId, datetime],
    queryFn: () => fetchForecast(checkpointId!, datetime!),
    enabled: !!checkpointId && !!datetime,
    staleTime: 60_000, // 1 minute — forecasts refresh frequently
  });
}

/** Result from useRaceForecast, including staleness metadata. */
export interface RaceForecastResult {
  data: RaceForecastResponse;
  stale: boolean;
}

/** Fetch forecasts for all checkpoints in a race (with stale header). */
export function useRaceForecast(
  raceId: string | null,
  targetDurationHours: number
) {
  return useQuery({
    queryKey: ["raceForecast", raceId, targetDurationHours],
    queryFn: () => fetchRaceForecastWithStale(raceId!, targetDurationHours),
    enabled: !!raceId,
    staleTime: 60_000, // 1 minute
    select: (response): RaceForecastResult => ({
      data: response.data,
      stale: response.stale,
    }),
  });
}
