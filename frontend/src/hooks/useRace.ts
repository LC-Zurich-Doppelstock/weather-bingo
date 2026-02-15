import { useQuery } from "@tanstack/react-query";
import { fetchRaces, fetchCourse, fetchCheckpoints } from "../api/client";

/** Fetch all available races. Races rarely change — cache for 5 minutes. */
export function useRaces() {
  return useQuery({
    queryKey: ["races"],
    queryFn: fetchRaces,
    staleTime: 5 * 60_000,
  });
}

/** Fetch course coordinates for a race. Course GPS data is static — cache for 10 minutes. */
export function useCourse(raceId: string | null) {
  return useQuery({
    queryKey: ["course", raceId],
    queryFn: () => fetchCourse(raceId!),
    enabled: !!raceId,
    staleTime: 10 * 60_000,
  });
}

/** Fetch checkpoints for a race. Checkpoints are static — cache for 10 minutes. */
export function useCheckpoints(raceId: string | null) {
  return useQuery({
    queryKey: ["checkpoints", raceId],
    queryFn: () => fetchCheckpoints(raceId!),
    enabled: !!raceId,
    staleTime: 10 * 60_000,
  });
}
