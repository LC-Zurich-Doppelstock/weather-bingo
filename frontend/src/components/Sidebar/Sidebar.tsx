import { useMemo } from "react";
import type { Checkpoint, Race } from "../../api/types";
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

  // Always fetch race forecast — we need expected_time for checkpoint detail view too
  const {
    data: raceForecast,
    isLoading: raceForecastLoading,
    isError: raceForecastError,
    refetch: refetchRaceForecast,
  } = useRaceForecast(race?.id ?? null, targetDurationHours);

  // Look up expected_time from the race forecast response (server-computed pacing)
  const passTime = useMemo(() => {
    if (!selectedCheckpointId || !raceForecast) return null;
    const cp = raceForecast.checkpoints.find(
      (c) => c.checkpoint_id === selectedCheckpointId
    );
    return cp?.expected_time ?? null;
  }, [selectedCheckpointId, raceForecast]);

  const {
    data: forecast,
    isLoading: forecastLoading,
    isError: forecastError,
    refetch: refetchForecast,
  } = useForecast(selectedCheckpointId, passTime);

  // No race selected
  if (!race) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <p className="text-text-muted">Select a race to view weather data</p>
      </div>
    );
  }

  // Checkpoint selected -> show detail view
  if (selectedCheckpoint) {
    // Still waiting for race forecast to resolve expected_time
    if (!passTime && raceForecastLoading) {
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
            passTime=""
            forecast={null}
            isLoading={true}
          />
        </div>
      );
    }

    // Race forecast failed — can't determine expected_time
    if (!passTime && raceForecastError) {
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
          <div className="p-4">
            <p className="text-error">Failed to load forecast data.</p>
            <button
              onClick={() => refetchRaceForecast()}
              className="mt-2 text-sm text-primary hover:text-primary-hover transition-colors"
            >
              Retry
            </button>
          </div>
        </div>
      );
    }

    // Forecast error (race forecast loaded fine but individual forecast failed)
    if (forecastError && !forecastLoading) {
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
          <div className="p-4">
            <p className="text-error">Failed to load forecast data.</p>
            <button
              onClick={() => refetchForecast()}
              className="mt-2 text-sm text-primary hover:text-primary-hover transition-colors"
            >
              Retry
            </button>
          </div>
        </div>
      );
    }

    if (passTime) {
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
  }

  // Race forecast error on course overview
  if (raceForecastError) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6">
        <p className="text-error">Failed to load forecast data.</p>
        <button
          onClick={() => refetchRaceForecast()}
          className="text-sm text-primary hover:text-primary-hover transition-colors"
        >
          Retry
        </button>
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
