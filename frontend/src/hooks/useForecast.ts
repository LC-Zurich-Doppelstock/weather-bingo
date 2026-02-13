import { useQuery } from "@tanstack/react-query";
import {
  fetchForecast,
  fetchForecastHistory,
  fetchRaceForecast,
} from "../api/client";

/** Fetch the latest forecast for a checkpoint at a specific time. */
export function useForecast(checkpointId: string | null, datetime: string | null) {
  return useQuery({
    queryKey: ["forecast", checkpointId, datetime],
    queryFn: () => fetchForecast(checkpointId!, datetime!),
    enabled: !!checkpointId && !!datetime,
  });
}

/** Fetch forecast history for a checkpoint. */
export function useForecastHistory(
  checkpointId: string | null,
  datetime: string | null
) {
  return useQuery({
    queryKey: ["forecastHistory", checkpointId, datetime],
    queryFn: () => fetchForecastHistory(checkpointId!, datetime!),
    enabled: !!checkpointId && !!datetime,
  });
}

/** Fetch forecasts for all checkpoints in a race. */
export function useRaceForecast(
  raceId: string | null,
  targetDurationHours: number
) {
  return useQuery({
    queryKey: ["raceForecast", raceId, targetDurationHours],
    queryFn: () => fetchRaceForecast(raceId!, targetDurationHours),
    enabled: !!raceId,
  });
}
