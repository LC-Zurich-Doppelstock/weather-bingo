import { useQuery } from "@tanstack/react-query";
import { fetchRaces, fetchCourse, fetchCheckpoints } from "../api/client";

/** Fetch all available races. */
export function useRaces() {
  return useQuery({
    queryKey: ["races"],
    queryFn: fetchRaces,
  });
}

/** Fetch course coordinates for a race. */
export function useCourse(raceId: string | null) {
  return useQuery({
    queryKey: ["course", raceId],
    queryFn: () => fetchCourse(raceId!),
    enabled: !!raceId,
  });
}

/** Fetch checkpoints for a race. */
export function useCheckpoints(raceId: string | null) {
  return useQuery({
    queryKey: ["checkpoints", raceId],
    queryFn: () => fetchCheckpoints(raceId!),
    enabled: !!raceId,
  });
}
