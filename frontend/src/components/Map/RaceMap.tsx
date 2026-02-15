import { useEffect, useRef } from "react";
import { MapContainer, TileLayer, useMap } from "react-leaflet";
import type { LatLngBoundsExpression } from "leaflet";
import "leaflet/dist/leaflet.css";

import CoursePolyline from "./CoursePolyline";
import CheckpointMarker from "./CheckpointMarker";
import type { Checkpoint, CoursePoint } from "../../api/types";

interface RaceMapProps {
  /** Pre-parsed course coordinates from the API. */
  course: CoursePoint[] | null;
  /** Checkpoints along the course. */
  checkpoints: Checkpoint[];
  /** Currently selected checkpoint ID. */
  selectedCheckpointId: string | null;
  /** Currently hovered checkpoint ID (from chart or map). */
  hoveredCheckpointId: string | null;
  /** Callback when a checkpoint marker is clicked. */
  onCheckpointSelect: (id: string) => void;
  /** Callback when a checkpoint marker is hovered/unhovered. */
  onCheckpointHover: (id: string | null) => void;
}

/** Dark-themed tile layer URL (CartoDB Dark Matter). */
const DARK_TILE_URL =
  "https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png";

const DARK_TILE_ATTRIBUTION =
  '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors &copy; <a href="https://carto.com/">CARTO</a>';

/** Default center (central Sweden) and zoom for when no course is loaded. */
const DEFAULT_CENTER: [number, number] = [61.5, 14.5];
const DEFAULT_ZOOM = 7;

/**
 * Sub-component that fits the map bounds once per race.
 * Uses a ref to track the last set of checkpoint IDs so it only re-fits
 * when the actual race changes, not on every render or re-fetch.
 */
function FitBounds({ checkpoints }: { checkpoints: Checkpoint[] }) {
  const map = useMap();
  const fittedIdsRef = useRef<string | null>(null);

  useEffect(() => {
    if (checkpoints.length === 0) return;

    // Build a stable fingerprint from sorted checkpoint IDs
    const fingerprint = checkpoints
      .map((cp) => cp.id)
      .sort()
      .join(",");

    // Only re-fit if the set of checkpoints actually changed (new race)
    if (fingerprint === fittedIdsRef.current) return;
    fittedIdsRef.current = fingerprint;

    const bounds: LatLngBoundsExpression = checkpoints.map(
      (cp) => [cp.latitude, cp.longitude] as [number, number]
    );

    map.fitBounds(bounds, { padding: [40, 40], maxZoom: 12 });
  }, [checkpoints, map]);

  return null;
}

/**
 * Interactive Leaflet map showing the race course and checkpoint markers.
 * Uses CartoDB Dark Matter tiles for the dark theme.
 */
export default function RaceMap({
  course,
  checkpoints,
  selectedCheckpointId,
  hoveredCheckpointId,
  onCheckpointSelect,
  onCheckpointHover,
}: RaceMapProps) {
  return (
    <div role="application" aria-label="Race course map" className="h-full w-full">
      <MapContainer
        center={DEFAULT_CENTER}
        zoom={DEFAULT_ZOOM}
        className="h-full w-full"
        zoomControl={true}
        attributionControl={true}
      >
      <TileLayer url={DARK_TILE_URL} attribution={DARK_TILE_ATTRIBUTION} />

      <FitBounds checkpoints={checkpoints} />

      {course && course.length > 0 && <CoursePolyline points={course} />}

      {checkpoints.map((cp) => (
        <CheckpointMarker
          key={cp.id}
          checkpoint={cp}
          isSelected={cp.id === selectedCheckpointId}
          isHovered={cp.id === hoveredCheckpointId}
          onClick={onCheckpointSelect}
          onHover={onCheckpointHover}
        />
      ))}
    </MapContainer>
    </div>
  );
}
