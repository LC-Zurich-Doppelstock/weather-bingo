import { useEffect } from "react";
import { useRaces } from "../../hooks/useRace";

interface RaceSelectorProps {
  selectedRaceId: string | null;
  onSelect: (raceId: string) => void;
}

export default function RaceSelector({
  selectedRaceId,
  onSelect,
}: RaceSelectorProps) {
  const { data: races, isLoading, isError } = useRaces();

  // Auto-select the first race when races load and nothing is selected
  useEffect(() => {
    if (!selectedRaceId && races && races.length > 0) {
      const firstRace = races[0];
      if (firstRace) {
        onSelect(firstRace.id);
      }
    }
  }, [races, selectedRaceId, onSelect]);

  if (isLoading) {
    return (
      <select
        disabled
        className="rounded border border-border bg-surface-alt px-3 py-1.5 text-sm text-text-muted"
      >
        <option>Loading races...</option>
      </select>
    );
  }

  if (isError || !races) {
    return (
      <select
        disabled
        className="rounded border border-error bg-surface-alt px-3 py-1.5 text-sm text-error"
      >
        <option>Failed to load races</option>
      </select>
    );
  }

  return (
    <select
      value={selectedRaceId ?? ""}
      onChange={(e) => onSelect(e.target.value)}
      className="rounded border border-border bg-surface-alt px-3 py-1.5 text-sm text-text-primary focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
      aria-label="Select a race"
    >
      {races.map((race) => (
        <option key={race.id} value={race.id}>
          {race.name} {race.year}
        </option>
      ))}
    </select>
  );
}
