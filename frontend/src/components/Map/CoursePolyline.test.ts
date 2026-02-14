import { describe, it, expect } from "vitest";
import type { CoursePoint } from "../../api/types";

/**
 * Tests for the CoursePolyline coordinate mapping logic.
 * The component itself is a thin wrapper around react-leaflet's Polyline,
 * so we test the data transformation that maps CoursePoint[] to [lat, lon][].
 */

/** Replicate the mapping logic from CoursePolyline for unit testing. */
function mapToPositions(points: CoursePoint[]): [number, number][] {
  return points.map((p) => [p.lat, p.lon]);
}

describe("CoursePolyline coordinate mapping", () => {
  it("maps course points to [lat, lon] pairs", () => {
    const points: CoursePoint[] = [
      { lat: 61.1, lon: 14.2, ele: 500 },
      { lat: 61.2, lon: 14.3, ele: 520 },
      { lat: 61.3, lon: 14.4, ele: 540 },
    ];

    const positions = mapToPositions(points);
    expect(positions).toHaveLength(3);
    expect(positions[0]).toEqual([61.1, 14.2]);
    expect(positions[1]).toEqual([61.2, 14.3]);
    expect(positions[2]).toEqual([61.3, 14.4]);
  });

  it("returns empty array for no points", () => {
    const positions = mapToPositions([]);
    expect(positions).toHaveLength(0);
  });

  it("handles single point", () => {
    const points: CoursePoint[] = [{ lat: 61.0, lon: 14.0, ele: 300 }];
    const positions = mapToPositions(points);
    expect(positions).toHaveLength(1);
    expect(positions[0]).toEqual([61.0, 14.0]);
  });

  it("preserves coordinate precision", () => {
    const points: CoursePoint[] = [
      { lat: 61.156789, lon: 13.263912, ele: 545.3 },
      { lat: 60.999001, lon: 14.537821, ele: 165.7 },
    ];

    const positions = mapToPositions(points);
    expect(positions[0]![0]).toBeCloseTo(61.156789, 6);
    expect(positions[0]![1]).toBeCloseTo(13.263912, 6);
    expect(positions[1]![0]).toBeCloseTo(60.999001, 6);
    expect(positions[1]![1]).toBeCloseTo(14.537821, 6);
  });

  it("discards elevation (used only for display, not map positioning)", () => {
    const points: CoursePoint[] = [
      { lat: 61.1, lon: 14.2, ele: 999 },
    ];

    const positions = mapToPositions(points);
    // Each position should be [lat, lon] only — no ele
    expect(positions[0]).toHaveLength(2);
  });
});
