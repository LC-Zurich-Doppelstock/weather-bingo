import { memo, useCallback } from "react";
import { CircleMarker, Tooltip } from "react-leaflet";
import type { Checkpoint } from "../../api/types";
import { colors } from "../../styles/theme";

interface CheckpointMarkerProps {
  /** Checkpoint data. */
  checkpoint: Checkpoint;
  /** Whether this checkpoint is currently selected. */
  isSelected: boolean;
  /** Callback when the marker is clicked. */
  onClick: (id: string) => void;
}

/**
 * A clickable circle marker for a checkpoint on the race map.
 * Shows a tooltip with the checkpoint name and distance.
 * Uses primary colour when selected, secondary when not.
 */
const CheckpointMarker = memo(function CheckpointMarker({
  checkpoint,
  isSelected,
  onClick,
}: CheckpointMarkerProps) {
  const handleClick = useCallback(() => {
    onClick(checkpoint.id);
  }, [checkpoint.id, onClick]);

  return (
    <CircleMarker
      center={[checkpoint.latitude, checkpoint.longitude]}
      radius={isSelected ? 8 : 6}
      pathOptions={{
        color: isSelected ? colors.primary : colors.secondary,
        fillColor: isSelected ? colors.primary : colors.surface,
        fillOpacity: isSelected ? 1 : 0.8,
        weight: isSelected ? 3 : 2,
      }}
      eventHandlers={{
        click: handleClick,
      }}
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
