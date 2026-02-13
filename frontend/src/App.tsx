import { useCallback, useEffect, useMemo, useState } from "react";
import Header from "./components/Layout/Header";
import Footer from "./components/Layout/Footer";
import TargetTimeInput from "./components/Controls/TargetTimeInput";
import RaceMap from "./components/Map/RaceMap";
import Sidebar from "./components/Sidebar/Sidebar";
import { useRace, useCheckpoints } from "./hooks/useRace";

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

  const { data: race } = useRace(selectedRaceId);
  const { data: checkpoints } = useCheckpoints(selectedRaceId);

  // Derive slider min/max from race distance
  const { minDuration, maxDuration, defaultDuration } = useMemo(() => {
    const distance = race?.distance_km ?? 90;
    // Round to nearest 0.5h for clean slider steps
    const min = Math.floor((distance / MAX_SPEED_KMH) * 2) / 2;
    const max = Math.ceil((distance / MIN_SPEED_KMH) * 2) / 2;
    const def = Math.round((distance / DEFAULT_SPEED_KMH) * 2) / 2;
    return { minDuration: Math.max(min, 1), maxDuration: max, defaultDuration: def };
  }, [race?.distance_km]);

  // Set default target duration when race loads (or changes)
  useEffect(() => {
    setTargetDuration(defaultDuration);
  }, [defaultDuration]);

  const effectiveDuration = targetDuration ?? defaultDuration;

  // Clamp target duration when race changes
  const clampedDuration = Math.min(Math.max(effectiveDuration, minDuration), maxDuration);

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
        <div className="relative min-h-0 flex-1 bg-surface-alt">
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
          <div className="h-64 lg:h-full">
            <RaceMap
              courseGpx={race?.course_gpx ?? null}
              checkpoints={checkpoints ?? []}
              selectedCheckpointId={selectedCheckpointId}
              onCheckpointSelect={setSelectedCheckpointId}
            />
          </div>
        </div>

        {/* Sidebar */}
        <aside className="w-full border-l border-border bg-surface lg:w-96 lg:overflow-y-auto" aria-label="Weather forecast sidebar">
          <Sidebar
            race={race ?? null}
            checkpoints={checkpoints ?? []}
            selectedCheckpointId={selectedCheckpointId}
            targetDurationHours={clampedDuration}
            onClearSelection={handleClearSelection}
          />
        </aside>
      </main>
      <Footer />
    </div>
  );
}

export default App;
