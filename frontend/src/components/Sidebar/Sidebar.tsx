import type { Checkpoint, Race } from "../../api/types";
import { usePassThroughTime } from "../../hooks/usePassThroughTime";
import { useForecast, useRaceForecast } from "../../hooks/useForecast";
import CheckpointDetail from "./CheckpointDetail";
import CourseOverview from "./CourseOverview";

interface SidebarProps {
  /** Currently selected race. */
  race: Race | null;
  /** All checkpoints for the race. */
  checkpoints: Checkpoint[];
  /** Currently selected checkpoint ID (null = course overview). */
  selectedCheckpointId: string | null;
  /** Currently hovered checkpoint ID (from chart or map). */
  hoveredCheckpointId: string | null;
  /** Target race duration in hours. */
  targetDurationHours: number;
  /** Callback to clear checkpoint selection (show course overview). */
  onClearSelection: () => void;
  /** Callback when a checkpoint is hovered/unhovered on the chart. */
  onCheckpointHover: (id: string | null) => void;
}

/**
 * Sidebar container that switches between checkpoint detail view
 * and course overview based on selection state.
 */
export default function Sidebar({
  race,
  checkpoints,
  selectedCheckpointId,
  hoveredCheckpointId,
  targetDurationHours,
  onClearSelection,
  onCheckpointHover,
}: SidebarProps) {
  const selectedCheckpoint =
    checkpoints.find((cp) => cp.id === selectedCheckpointId) ?? null;

  const passTime = usePassThroughTime(
    selectedCheckpoint,
    race,
    targetDurationHours
  );

  const { data: forecast, isLoading: forecastLoading } = useForecast(
    selectedCheckpointId,
    passTime
  );

  const { data: raceForecast, isLoading: raceForecastLoading } =
    useRaceForecast(
      !selectedCheckpointId ? race?.id ?? null : null,
      targetDurationHours
    );

  // No race selected
  if (!race) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <p className="text-text-muted">Select a race to view weather data</p>
      </div>
    );
  }

  // Checkpoint selected -> show detail view
  if (selectedCheckpoint && passTime) {
    return (
      <div className="h-full overflow-y-auto" role="region" aria-label={`Weather details for ${selectedCheckpoint.name}`}>
        <div className="sticky top-0 z-10 border-b border-border bg-surface p-3">
          <button
            onClick={onClearSelection}
            className="text-sm text-text-secondary hover:text-primary transition-colors"
            aria-label="Back to course overview"
          >
            &larr; Course Overview
          </button>
        </div>
        <CheckpointDetail
          checkpoint={selectedCheckpoint}
          passTime={passTime}
          forecast={forecast ?? null}
          isLoading={forecastLoading}
        />
      </div>
    );
  }

  // No checkpoint selected -> show course overview
  return (
    <div className="h-full overflow-y-auto" role="region" aria-label="Course weather overview">
      <CourseOverview
        raceForecast={raceForecast ?? null}
        checkpoints={checkpoints}
        isLoading={raceForecastLoading}
        hoveredCheckpointId={hoveredCheckpointId}
        onCheckpointHover={onCheckpointHover}
      />
    </div>
  );
}
