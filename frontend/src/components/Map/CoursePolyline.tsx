import { memo, useMemo } from "react";
import { Polyline } from "react-leaflet";
import type { LatLngExpression } from "leaflet";
import { colors } from "../../styles/theme";
import type { CoursePoint } from "../../api/types";

interface CoursePolylineProps {
  /** Pre-parsed course coordinates from the API. */
  points: CoursePoint[];
}

/**
 * Renders the race course as a polyline on the Leaflet map.
 * Accepts pre-parsed JSON coordinates instead of raw GPX.
 */
const CoursePolyline = memo(function CoursePolyline({ points }: CoursePolylineProps) {
  const positions: LatLngExpression[] = useMemo(
    () => points.map((p) => [p.lat, p.lon] as [number, number]),
    [points]
  );

  if (positions.length < 2) {
    return null;
  }

  return (
    <Polyline
      positions={positions}
      pathOptions={{
        color: colors.accentRose,
        weight: 3,
        opacity: 0.8,
        lineCap: "round",
        lineJoin: "round",
      }}
    />
  );
});

export default CoursePolyline;
