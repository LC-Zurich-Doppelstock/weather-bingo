/**
 * Even pacing model for calculating checkpoint pass-through times.
 * See specs.md Section 10.1.
 *
 * pass_through_time(checkpoint) =
 *   race.start_time + target_duration * (checkpoint.distance_km / race.distance_km)
 */

/**
 * Calculate the expected pass-through time for a checkpoint.
 *
 * @param startTime - Race start time (ISO 8601 string)
 * @param distanceKm - Checkpoint distance from start (km)
 * @param totalDistanceKm - Total race distance (km)
 * @param durationHours - Target finish duration (hours)
 * @returns ISO 8601 string of expected pass-through time
 */
export function calculatePassTime(
  startTime: string,
  distanceKm: number,
  totalDistanceKm: number,
  durationHours: number
): string {
  const start = new Date(startTime);
  const durationMs = durationHours * 60 * 60 * 1000;
  const fraction = distanceKm / totalDistanceKm;
  const passMs = start.getTime() + durationMs * fraction;
  return new Date(passMs).toISOString();
}

/**
 * Calculate all checkpoint pass-through times for a race.
 */
export function calculateAllPassTimes(
  startTime: string,
  totalDistanceKm: number,
  durationHours: number,
  checkpoints: { distance_km: number }[]
): string[] {
  return checkpoints.map((cp) =>
    calculatePassTime(startTime, cp.distance_km, totalDistanceKm, durationHours)
  );
}
