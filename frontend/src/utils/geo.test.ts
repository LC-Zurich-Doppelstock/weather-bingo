import { describe, it, expect } from "vitest";
import { haversineDistance, computeElevationProfile } from "./geo";
import type { CoursePoint } from "../api/types";

describe("haversineDistance", () => {
  it("returns 0 for identical points", () => {
    expect(haversineDistance(60, 15, 60, 15)).toBe(0);
  });

  it("computes ~111 km for 1 degree of latitude at the equator", () => {
    const d = haversineDistance(0, 0, 1, 0);
    // 1 degree latitude ≈ 111.19 km
    expect(d).toBeGreaterThan(110);
    expect(d).toBeLessThan(112);
  });

  it("computes ~111 km for 1 degree of longitude at the equator", () => {
    const d = haversineDistance(0, 0, 0, 1);
    expect(d).toBeGreaterThan(110);
    expect(d).toBeLessThan(112);
  });

  it("computes shorter distance for 1 degree longitude at 60°N", () => {
    // At 60°N, 1 degree longitude ≈ 55.6 km
    const d = haversineDistance(60, 0, 60, 1);
    expect(d).toBeGreaterThan(54);
    expect(d).toBeLessThan(57);
  });

  it("handles negative coordinates", () => {
    const d = haversineDistance(-33.8688, 151.2093, -37.8136, 144.9631);
    // Sydney to Melbourne ≈ 714 km
    expect(d).toBeGreaterThan(700);
    expect(d).toBeLessThan(730);
  });

  it("handles antipodal points", () => {
    const d = haversineDistance(0, 0, 0, 180);
    // Half circumference ≈ 20015 km
    expect(d).toBeGreaterThan(20000);
    expect(d).toBeLessThan(20100);
  });
});

describe("computeElevationProfile", () => {
  it("returns empty array for empty input", () => {
    expect(computeElevationProfile([])).toEqual([]);
  });

  it("returns single point with distance 0 for one-element input", () => {
    const points: CoursePoint[] = [{ lat: 60, lon: 15, ele: 300 }];
    const result = computeElevationProfile(points);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ distance_km: 0, ele: 300 });
  });

  it("computes cumulative distance for multiple points", () => {
    const points: CoursePoint[] = [
      { lat: 0, lon: 0, ele: 100 },
      { lat: 1, lon: 0, ele: 200 },
      { lat: 2, lon: 0, ele: 150 },
    ];
    const result = computeElevationProfile(points);

    expect(result).toHaveLength(3);
    expect(result[0]!.distance_km).toBe(0);
    expect(result[0]!.ele).toBe(100);

    // First segment: ~111 km
    expect(result[1]!.distance_km).toBeGreaterThan(110);
    expect(result[1]!.distance_km).toBeLessThan(112);
    expect(result[1]!.ele).toBe(200);

    // Second segment: cumulative ~222 km
    expect(result[2]!.distance_km).toBeGreaterThan(220);
    expect(result[2]!.distance_km).toBeLessThan(224);
    expect(result[2]!.ele).toBe(150);
  });

  it("distances are monotonically increasing", () => {
    const points: CoursePoint[] = [
      { lat: 61.0, lon: 14.0, ele: 400 },
      { lat: 61.1, lon: 14.1, ele: 450 },
      { lat: 61.2, lon: 14.2, ele: 350 },
      { lat: 61.3, lon: 14.3, ele: 500 },
    ];
    const result = computeElevationProfile(points);

    for (let i = 1; i < result.length; i++) {
      expect(result[i]!.distance_km).toBeGreaterThan(result[i - 1]!.distance_km);
    }
  });

  it("preserves elevation values from input", () => {
    const points: CoursePoint[] = [
      { lat: 0, lon: 0, ele: 10 },
      { lat: 0, lon: 0.001, ele: 20 },
      { lat: 0, lon: 0.002, ele: 5 },
    ];
    const result = computeElevationProfile(points);
    expect(result.map((p) => p.ele)).toEqual([10, 20, 5]);
  });
});
