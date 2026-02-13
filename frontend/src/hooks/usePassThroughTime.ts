import { useMemo } from "react";
import { calculatePassTime } from "../utils/pacing";
import type { Checkpoint, Race } from "../api/types";

/**
 * Calculate the expected pass-through time for a checkpoint
 * based on race start time, distance, and target duration.
 */
export function usePassThroughTime(
  checkpoint: Checkpoint | null,
  race: Race | null,
  durationHours: number
): string | null {
  return useMemo(() => {
    if (!checkpoint || !race) return null;
    return calculatePassTime(
      race.start_time,
      checkpoint.distance_km,
      race.distance_km,
      durationHours
    );
  }, [checkpoint, race, durationHours]);
}
