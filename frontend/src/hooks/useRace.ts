import { useQuery } from "@tanstack/react-query";
import { fetchRaces, fetchCourse, fetchCheckpoints } from "../api/client";
import type { CoursePoint } from "../api/types";

/** Fetch all available races. Races rarely change — cache for 5 minutes. */
export function useRaces() {
  return useQuery({
    queryKey: ["races"],
    queryFn: fetchRaces,
    staleTime: 5 * 60_000,
  });
}

/**
 * Fetch course coordinates for a race, with cumulative distances
 * and pacing time fractions.
 *
 * Each CoursePoint always includes a `time_fraction` field (0.0 at start,
 * 1.0 at finish) based on elevation-adjusted pacing. The fractions are
 * duration-independent — they represent relative effort, not clock time.
 * Course GPS data is static — cache for 10 minutes.
 */
export function useCourse(raceId: string | null) {
  return useQuery<CoursePoint[]>({
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
