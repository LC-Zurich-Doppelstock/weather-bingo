import { describe, it, expect } from "vitest";
import { parseGpxTrack } from "./CoursePolyline";

describe("parseGpxTrack", () => {
  it("parses track points from a GPX string", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1">
        <trk><trkseg>
          <trkpt lat="61.1" lon="14.2"><ele>500</ele></trkpt>
          <trkpt lat="61.2" lon="14.3"><ele>520</ele></trkpt>
          <trkpt lat="61.3" lon="14.4"><ele>540</ele></trkpt>
        </trkseg></trk>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(3);
    expect(points[0]).toEqual([61.1, 14.2]);
    expect(points[1]).toEqual([61.2, 14.3]);
    expect(points[2]).toEqual([61.3, 14.4]);
  });

  it("parses route points when no track points exist", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1">
        <rte>
          <rtept lat="61.0" lon="14.0"/>
          <rtept lat="61.5" lon="14.5"/>
        </rte>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(2);
    expect(points[0]).toEqual([61.0, 14.0]);
    expect(points[1]).toEqual([61.5, 14.5]);
  });

  it("falls back to waypoints when no tracks or routes exist", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1">
        <wpt lat="61.15" lon="14.25"><name>Start</name></wpt>
        <wpt lat="61.35" lon="14.45"><name>Finish</name></wpt>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(2);
    expect(points[0]).toEqual([61.15, 14.25]);
    expect(points[1]).toEqual([61.35, 14.45]);
  });

  it("prefers track points over route points and waypoints", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1">
        <wpt lat="0" lon="0"><name>WP</name></wpt>
        <rte><rtept lat="1" lon="1"/></rte>
        <trk><trkseg>
          <trkpt lat="61.1" lon="14.2"/>
          <trkpt lat="61.2" lon="14.3"/>
        </trkseg></trk>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(2);
    expect(points[0]).toEqual([61.1, 14.2]);
  });

  it("returns empty array for GPX with no points", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1"></gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(0);
  });

  it("handles GPX with missing lat/lon attributes gracefully", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1">
        <trk><trkseg>
          <trkpt lat="61.1" lon="14.2"/>
          <trkpt lon="14.3"/>
          <trkpt lat="61.3" lon="14.4"/>
        </trkseg></trk>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    // Missing lat attribute -> getAttribute returns null -> fallback "0" -> parsed as 0
    // So 3 points are returned (0 is a valid number)
    expect(points).toHaveLength(3);
    expect(points[0]).toEqual([61.1, 14.2]);
    expect(points[2]).toEqual([61.3, 14.4]);
  });

  it("handles a real-world GPX structure with multiple track segments", () => {
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
      <gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1">
        <trk>
          <name>Vasaloppet 2026</name>
          <trkseg>
            <trkpt lat="61.1567" lon="13.2639"><ele>545</ele></trkpt>
            <trkpt lat="61.1600" lon="13.2700"><ele>550</ele></trkpt>
          </trkseg>
          <trkseg>
            <trkpt lat="61.1700" lon="13.2800"><ele>530</ele></trkpt>
            <trkpt lat="61.1800" lon="13.2900"><ele>520</ele></trkpt>
          </trkseg>
        </trk>
      </gpx>`;

    const points = parseGpxTrack(gpx);
    expect(points).toHaveLength(4);
    expect(points[0]![0]).toBeCloseTo(61.1567, 4);
    expect(points[0]![1]).toBeCloseTo(13.2639, 4);
  });
});
