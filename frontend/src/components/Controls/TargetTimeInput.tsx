import { formatDuration } from "../../utils/formatting";

interface TargetTimeInputProps {
  value: number; // hours
  onChange: (hours: number) => void;
  /** Minimum duration in hours (derived from race distance / max speed). */
  min: number;
  /** Maximum duration in hours (derived from race distance / min speed). */
  max: number;
  step?: number;
}

export default function TargetTimeInput({
  value,
  onChange,
  min,
  max,
  step = 0.5,
}: TargetTimeInputProps) {
  return (
    <div className="flex items-center gap-3">
      <label htmlFor="target-time" className="text-sm text-text-secondary">
        Target time
      </label>
      <input
        id="target-time"
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="h-2 flex-1 cursor-pointer appearance-none rounded-lg bg-surface-alt accent-accent-rose"
        aria-label="Target race duration"
        aria-valuetext={formatDuration(value)}
      />
      <span className="min-w-[4rem] text-right text-sm font-medium text-text-primary">
        {formatDuration(value)}
      </span>
    </div>
  );
}
