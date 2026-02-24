import { describe, it, expect } from "vitest";
import { computeElevationProfile } from "./geo";
import type { CoursePoint } from "../api/types";

describe("computeElevationProfile", () => {
  it("returns empty array for empty input", () => {
    expect(computeElevationProfile([])).toEqual([]);
  });

  it("returns single point preserving distance_km for one-element input", () => {
    const points: CoursePoint[] = [{ lat: 60, lon: 15, ele: 300, distance_km: 0, time_fraction: 0 }];
    const result = computeElevationProfile(points);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ distance_km: 0, ele: 300 });
  });

  it("maps server-provided distance_km for multiple points", () => {
    const points: CoursePoint[] = [
      { lat: 0, lon: 0, ele: 100, distance_km: 0, time_fraction: 0 },
      { lat: 1, lon: 0, ele: 200, distance_km: 111.19, time_fraction: 0.5 },
      { lat: 2, lon: 0, ele: 150, distance_km: 222.39, time_fraction: 1 },
    ];
    const result = computeElevationProfile(points);

    expect(result).toHaveLength(3);
    expect(result[0]!.distance_km).toBe(0);
    expect(result[0]!.ele).toBe(100);

    expect(result[1]!.distance_km).toBe(111.19);
    expect(result[1]!.ele).toBe(200);

    expect(result[2]!.distance_km).toBe(222.39);
    expect(result[2]!.ele).toBe(150);
  });

  it("distances are monotonically increasing", () => {
    const points: CoursePoint[] = [
      { lat: 61.0, lon: 14.0, ele: 400, distance_km: 0, time_fraction: 0 },
      { lat: 61.1, lon: 14.1, ele: 450, distance_km: 12.5, time_fraction: 0.33 },
      { lat: 61.2, lon: 14.2, ele: 350, distance_km: 25.0, time_fraction: 0.67 },
      { lat: 61.3, lon: 14.3, ele: 500, distance_km: 37.5, time_fraction: 1 },
    ];
    const result = computeElevationProfile(points);

    for (let i = 1; i < result.length; i++) {
      expect(result[i]!.distance_km).toBeGreaterThan(result[i - 1]!.distance_km);
    }
  });

  it("preserves elevation values from input", () => {
    const points: CoursePoint[] = [
      { lat: 0, lon: 0, ele: 10, distance_km: 0, time_fraction: 0 },
      { lat: 0, lon: 0.001, ele: 20, distance_km: 0.111, time_fraction: 0.5 },
      { lat: 0, lon: 0.002, ele: 5, distance_km: 0.222, time_fraction: 1 },
    ];
    const result = computeElevationProfile(points);
    expect(result.map((p) => p.ele)).toEqual([10, 20, 5]);
  });
});
