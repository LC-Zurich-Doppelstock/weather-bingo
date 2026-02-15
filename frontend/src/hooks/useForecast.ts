import { useQuery } from "@tanstack/react-query";
import {
  fetchForecast,
  fetchRaceForecast,
} from "../api/client";

/** Fetch the latest forecast for a checkpoint at a specific time. */
export function useForecast(checkpointId: string | null, datetime: string | null) {
  return useQuery({
    queryKey: ["forecast", checkpointId, datetime],
    queryFn: () => fetchForecast(checkpointId!, datetime!),
    enabled: !!checkpointId && !!datetime,
    staleTime: 60_000, // 1 minute — forecasts refresh frequently
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
    staleTime: 60_000, // 1 minute
  });
}
