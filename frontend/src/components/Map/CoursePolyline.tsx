import { memo, useMemo } from "react";
import { Polyline } from "react-leaflet";
import type { LatLngExpression } from "leaflet";
import { colors } from "../../styles/theme";

interface CoursePolylineProps {
  /** Raw GPX XML string. */
  gpxData: string;
}

/**
 * Parse a GPX XML string and extract track points as [lat, lng] pairs.
 * Handles both <trkpt> (track) and <rtept> (route) elements.
 */
export function parseGpxTrack(gpxXml: string): [number, number][] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(gpxXml, "application/xml");

  const points: [number, number][] = [];

  // Try <trkpt> elements first (track points)
  const trkpts = doc.getElementsByTagName("trkpt");
  for (let i = 0; i < trkpts.length; i++) {
    const el = trkpts.item(i);
    if (!el) continue;
    const lat = parseFloat(el.getAttribute("lat") || "0");
    const lon = parseFloat(el.getAttribute("lon") || "0");
    if (!isNaN(lat) && !isNaN(lon)) {
      points.push([lat, lon]);
    }
  }

  // If no track points, try <rtept> elements (route points)
  if (points.length === 0) {
    const rtepts = doc.getElementsByTagName("rtept");
    for (let i = 0; i < rtepts.length; i++) {
      const el = rtepts.item(i);
      if (!el) continue;
      const lat = parseFloat(el.getAttribute("lat") || "0");
      const lon = parseFloat(el.getAttribute("lon") || "0");
      if (!isNaN(lat) && !isNaN(lon)) {
        points.push([lat, lon]);
      }
    }
  }

  // If still no points, try <wpt> (waypoints) as fallback
  if (points.length === 0) {
    const wpts = doc.getElementsByTagName("wpt");
    for (let i = 0; i < wpts.length; i++) {
      const el = wpts.item(i);
      if (!el) continue;
      const lat = parseFloat(el.getAttribute("lat") || "0");
      const lon = parseFloat(el.getAttribute("lon") || "0");
      if (!isNaN(lat) && !isNaN(lon)) {
        points.push([lat, lon]);
      }
    }
  }

  return points;
}

/**
 * Renders the race course as a polyline on the Leaflet map.
 * Parses GPX XML and draws the track in the primary colour.
 */
const CoursePolyline = memo(function CoursePolyline({ gpxData }: CoursePolylineProps) {
  const positions: LatLngExpression[] = useMemo(() => {
    return parseGpxTrack(gpxData);
  }, [gpxData]);

  if (positions.length < 2) {
    return null;
  }

  return (
    <Polyline
      positions={positions}
      pathOptions={{
        color: colors.primary,
        weight: 3,
        opacity: 0.8,
        lineCap: "round",
        lineJoin: "round",
      }}
    />
  );
});

export default CoursePolyline;
