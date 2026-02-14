import RaceSelector from "../Controls/RaceSelector";
import { formatDate } from "../../utils/formatting";

interface HeaderProps {
  selectedRaceId: string | null;
  onRaceSelect: (raceId: string) => void;
  /** Start time of the selected race (ISO 8601), for displaying race date. */
  raceStartTime: string | null;
}

export default function Header({
  selectedRaceId,
  onRaceSelect,
  raceStartTime,
}: HeaderProps) {
  return (
    <header className="flex items-center justify-between border-b border-border bg-surface px-4 py-3">
      <div className="flex items-baseline gap-3">
        <h1 className="text-lg font-bold text-accent-rose">Weather Bingo</h1>
        {raceStartTime && (
          <span className="text-sm text-text-secondary">
            {formatDate(raceStartTime)}
          </span>
        )}
      </div>
      <RaceSelector
        selectedRaceId={selectedRaceId}
        onSelect={onRaceSelect}
      />
    </header>
  );
}
