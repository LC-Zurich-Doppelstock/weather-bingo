import type { CoursePoint } from "../api/types";

export interface ElevationPoint {
  distance_km: number;
  ele: number;
}

/**
 * Compute elevation profile from course points.
 *
 * Uses the server-provided `distance_km` on each CoursePoint (cumulative
 * Haversine distance computed by the API). Returns an array suitable for
 * charting: `{ distance_km, ele }` per point.
 */
export function computeElevationProfile(
  points: CoursePoint[],
): ElevationPoint[] {
  if (points.length === 0) return [];

  return points.map((p) => ({
    distance_km: p.distance_km,
    ele: p.ele,
  }));
}
