import { useQuery } from "@tanstack/react-query";
import { fetchRaces, fetchRace, fetchCheckpoints } from "../api/client";

/** Fetch all available races. */
export function useRaces() {
  return useQuery({
    queryKey: ["races"],
    queryFn: fetchRaces,
  });
}

/** Fetch a single race with GPX data. */
export function useRace(raceId: string | null) {
  return useQuery({
    queryKey: ["race", raceId],
    queryFn: () => fetchRace(raceId!),
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
