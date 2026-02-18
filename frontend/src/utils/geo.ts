import type { CoursePoint } from "../api/types";

const EARTH_RADIUS_KM = 6371;

/**
 * Compute Haversine distance between two lat/lon points in kilometres.
 */
export function haversineDistance(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number,
): number {
  const toRad = (deg: number) => (deg * Math.PI) / 180;

  const dLat = toRad(lat2 - lat1);
  const dLon = toRad(lon2 - lon1);
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.sin(dLon / 2) ** 2;
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

  return EARTH_RADIUS_KM * c;
}

export interface ElevationPoint {
  distance_km: number;
  ele: number;
}

/**
 * Compute cumulative distance along a GPS track and pair with elevation.
 * Returns an array suitable for charting: { distance_km, ele } per point.
 *
 * Distance is computed via Haversine between consecutive points.
 * The first point always has distance_km = 0.
 */
export function computeElevationProfile(
  points: CoursePoint[],
): ElevationPoint[] {
  if (points.length === 0) return [];

  const result: ElevationPoint[] = [{ distance_km: 0, ele: points[0]!.ele }];
  let cumulative = 0;

  for (let i = 1; i < points.length; i++) {
    const prev = points[i - 1]!;
    const curr = points[i]!;
    cumulative += haversineDistance(prev.lat, prev.lon, curr.lat, curr.lon);
    result.push({ distance_km: cumulative, ele: curr.ele });
  }

  return result;
}
