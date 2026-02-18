import { useCallback, useEffect, useMemo, useState } from "react";
import Header from "./components/Layout/Header";
import Footer from "./components/Layout/Footer";
import TargetTimeInput from "./components/Controls/TargetTimeInput";
import RaceMap from "./components/Map/RaceMap";
import ElevationProfile from "./components/ElevationProfile/ElevationProfile";
import Sidebar from "./components/Sidebar/Sidebar";
import { useRaces, useCourse, useCheckpoints } from "./hooks/useRace";
import { useDebouncedValue } from "./hooks/useDebouncedValue";

/** Max skier speed in km/h (sprint pace). */
const MAX_SPEED_KMH = 30;
/** Min skier speed in km/h (leisurely pace). */
const MIN_SPEED_KMH = 10;
/** Default pace in km/h for initial slider position. */
const DEFAULT_SPEED_KMH = 15;

function App() {
  const [selectedRaceId, setSelectedRaceId] = useState<string | null>(null);
  const [targetDuration, setTargetDuration] = useState<number | null>(null);
  const [selectedCheckpointId, setSelectedCheckpointId] = useState<
    string | null
  >(null);
  const [hoveredCheckpointId, setHoveredCheckpointId] = useState<
    string | null
  >(null);

  const { data: races } = useRaces();
  const { data: course } = useCourse(selectedRaceId);
  const { data: checkpoints } = useCheckpoints(selectedRaceId);

  // Derive selected race from the races list (no separate detail endpoint)
  const race = useMemo(
    () => races?.find((r) => r.id === selectedRaceId) ?? null,
    [races, selectedRaceId]
  );

  // Derive slider min/max from race distance
  const { minDuration, maxDuration, defaultDuration } = useMemo(() => {
    const distance = race?.distance_km ?? 90;
    // Round to nearest 0.5h for clean slider steps
    const min = Math.floor((distance / MAX_SPEED_KMH) * 2) / 2;
    const max = Math.ceil((distance / MIN_SPEED_KMH) * 2) / 2;
    const def = Math.round((distance / DEFAULT_SPEED_KMH) * 2) / 2;
    return { minDuration: Math.max(min, 1), maxDuration: max, defaultDuration: def };
  }, [race?.distance_km]);

  // Clear checkpoint selection and reset duration when race changes
  useEffect(() => {
    setSelectedCheckpointId(null);
    setTargetDuration(defaultDuration);
  }, [defaultDuration]);

  const effectiveDuration = targetDuration ?? defaultDuration;

  // Clamp target duration when race changes and sync back to state
  const clampedDuration = Math.min(Math.max(effectiveDuration, minDuration), maxDuration);

  // Sync clamped value back to state so the slider stays consistent
  useEffect(() => {
    if (clampedDuration !== effectiveDuration) {
      setTargetDuration(clampedDuration);
    }
  }, [clampedDuration, effectiveDuration]);

  // Debounce the clamped duration to avoid hammering the API while the user drags the slider.
  // The slider still moves instantly (controlled by clampedDuration), but API fetches wait
  // for 300ms of inactivity.
  const debouncedDuration = useDebouncedValue(clampedDuration, 300);

  const handleClearSelection = useCallback(() => {
    setSelectedCheckpointId(null);
  }, []);

  return (
    <div className="flex h-screen flex-col bg-background text-text-primary">
      <Header
        selectedRaceId={selectedRaceId}
        onRaceSelect={setSelectedRaceId}
        raceStartTime={race?.start_time ?? null}
      />
      <main className="flex min-h-0 flex-1 flex-col lg:flex-row">
        {/* Map area */}
        <div className="relative min-h-0 lg:flex-[3] lg:flex lg:flex-col bg-surface-alt">
          {/* Target time control overlay */}
          <div className="absolute left-4 right-4 top-4 z-[1000] rounded-lg bg-surface/90 p-3 backdrop-blur-sm lg:left-auto lg:right-4 lg:w-80">
            <TargetTimeInput
              value={clampedDuration}
              onChange={setTargetDuration}
              min={minDuration}
              max={maxDuration}
            />
          </div>
          {/* Race map */}
          <div className="h-64 lg:flex-1 lg:min-h-0">
            <RaceMap
              course={course ?? null}
              checkpoints={checkpoints ?? []}
              selectedCheckpointId={selectedCheckpointId}
              hoveredCheckpointId={hoveredCheckpointId}
              onCheckpointSelect={setSelectedCheckpointId}
              onCheckpointHover={setHoveredCheckpointId}
            />
          </div>
          {/* Elevation profile (desktop only, collapsible) */}
          <ElevationProfile
            course={course ?? null}
            checkpoints={checkpoints ?? []}
            hoveredCheckpointId={hoveredCheckpointId}
            selectedCheckpointId={selectedCheckpointId}
            onCheckpointHover={setHoveredCheckpointId}
          />
        </div>

        {/* Sidebar */}
        <aside className="min-h-0 flex-1 overflow-y-auto border-l border-border bg-surface lg:flex-[2]" aria-label="Weather forecast sidebar">
          <Sidebar
            race={race ?? null}
            checkpoints={checkpoints ?? []}
            selectedCheckpointId={selectedCheckpointId}
            hoveredCheckpointId={hoveredCheckpointId}
            targetDurationHours={debouncedDuration}
            onClearSelection={handleClearSelection}
            onCheckpointHover={setHoveredCheckpointId}
          />
        </aside>
      </main>
      <Footer />
    </div>
  );
}

export default App;
