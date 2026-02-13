import { describe, it, expect } from "vitest";
import { calculatePassTime, calculateAllPassTimes } from "./pacing";

describe("calculatePassTime", () => {
  const startTime = "2026-03-01T08:00:00+01:00";
  const totalDistanceKm = 90;
  const durationHours = 8;

  it("returns start time for distance 0", () => {
    const result = calculatePassTime(startTime, 0, totalDistanceKm, durationHours);
    expect(new Date(result).getTime()).toBe(new Date(startTime).getTime());
  });

  it("returns finish time for full distance", () => {
    const result = calculatePassTime(startTime, 90, totalDistanceKm, durationHours);
    const expected = new Date(startTime).getTime() + 8 * 60 * 60 * 1000;
    expect(new Date(result).getTime()).toBe(expected);
  });

  it("calculates midpoint correctly", () => {
    const result = calculatePassTime(startTime, 45, totalDistanceKm, durationHours);
    const expected = new Date(startTime).getTime() + 4 * 60 * 60 * 1000;
    expect(new Date(result).getTime()).toBe(expected);
  });

  it("handles Smagan checkpoint (11 km)", () => {
    const result = calculatePassTime(startTime, 11, totalDistanceKm, durationHours);
    const expected =
      new Date(startTime).getTime() + (11 / 90) * 8 * 60 * 60 * 1000;
    expect(new Date(result).getTime()).toBeCloseTo(expected, -2);
  });
});

describe("calculateAllPassTimes", () => {
  it("returns correct number of times", () => {
    const checkpoints = [
      { distance_km: 0 },
      { distance_km: 11 },
      { distance_km: 24 },
      { distance_km: 90 },
    ];
    const times = calculateAllPassTimes(
      "2026-03-01T08:00:00+01:00",
      90,
      8,
      checkpoints
    );
    expect(times).toHaveLength(4);
  });
});
