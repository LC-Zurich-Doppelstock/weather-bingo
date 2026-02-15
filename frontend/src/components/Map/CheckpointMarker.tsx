import { memo, useCallback, useMemo, useRef } from "react";
import { CircleMarker, Tooltip } from "react-leaflet";
import type L from "leaflet";
import type { Checkpoint } from "../../api/types";
import { colors } from "../../styles/theme";

interface CheckpointMarkerProps {
  /** Checkpoint data. */
  checkpoint: Checkpoint;
  /** Whether this checkpoint is currently selected. */
  isSelected: boolean;
  /** Whether this checkpoint is currently hovered (from chart or map). */
  isHovered: boolean;
  /** Callback when the marker is clicked. */
  onClick: (id: string) => void;
  /** Callback when the marker is hovered/unhovered. */
  onHover: (id: string | null) => void;
}

/**
 * A clickable circle marker for a checkpoint on the race map.
 * Shows a tooltip with the checkpoint name and distance.
 * Uses primary colour when selected, secondary when not.
 */
const CheckpointMarker = memo(function CheckpointMarker({
  checkpoint,
  isSelected,
  isHovered,
  onClick,
  onHover,
}: CheckpointMarkerProps) {
  const markerRef = useRef<L.CircleMarker | null>(null);

  const handleClick = useCallback(() => {
    onClick(checkpoint.id);
  }, [checkpoint.id, onClick]);

  const handleMouseOver = useCallback(() => {
    onHover(checkpoint.id);
  }, [checkpoint.id, onHover]);

  const handleMouseOut = useCallback(() => {
    onHover(null);
  }, [onHover]);

  const handleKeyDown = useCallback(
    (e: L.LeafletKeyboardEvent) => {
      if (e.originalEvent.key === "Enter" || e.originalEvent.key === " ") {
        e.originalEvent.preventDefault();
        onClick(checkpoint.id);
      }
    },
    [checkpoint.id, onClick]
  );

  const highlighted = isSelected || isHovered;

  // Memoize to prevent Leaflet from re-binding on every render
  const pathOptions = useMemo(
    () => ({
      color: colors.accentRose,
      fillColor: highlighted ? colors.accentRose : colors.surface,
      fillOpacity: highlighted ? 1 : 0.8,
      weight: highlighted ? 3 : 2,
    }),
    [highlighted]
  );

  const eventHandlers = useMemo(
    () => ({
      click: handleClick,
      mouseover: handleMouseOver,
      mouseout: handleMouseOut,
      keydown: handleKeyDown,
      add: (e: L.LeafletEvent) => {
        // Make the SVG element focusable for keyboard navigation
        const el = (e.target as L.CircleMarker).getElement();
        if (el) {
          el.setAttribute("tabindex", "0");
          el.setAttribute("role", "button");
          el.setAttribute(
            "aria-label",
            `${checkpoint.name}, ${checkpoint.distance_km.toFixed(1)} km`
          );
        }
      },
    }),
    [handleClick, handleMouseOver, handleMouseOut, handleKeyDown, checkpoint.name, checkpoint.distance_km]
  );

  return (
    <CircleMarker
      ref={markerRef}
      center={[checkpoint.latitude, checkpoint.longitude]}
      radius={highlighted ? 8 : 6}
      pathOptions={pathOptions}
      eventHandlers={eventHandlers}
    >
      <Tooltip
        direction="top"
        offset={[0, -10]}
        className="checkpoint-tooltip"
      >
        <div className="text-xs font-medium">
          <div>{checkpoint.name}</div>
          <div className="text-text-secondary">
            {checkpoint.distance_km.toFixed(1)} km
          </div>
        </div>
      </Tooltip>
    </CircleMarker>
  );
});

export default CheckpointMarker;
