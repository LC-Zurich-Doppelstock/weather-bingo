/**
 * Formatting utilities for weather data display.
 */

/**
 * Race timezone. Vasaloppet is in Sweden; all race times are displayed
 * in this timezone. Extracted as a constant to avoid magic strings and
 * make it easy to support other races in different timezones later.
 */
export const RACE_TIMEZONE = "Europe/Stockholm";

/** Format temperature with degree sign. */
export function formatTemp(celsius: number): string {
  return `${Math.round(celsius)}°C`;
}

/** Format wind speed in m/s. */
export function formatWind(ms: number): string {
  return `${ms.toFixed(1)} m/s`;
}

/** Format precipitation in mm. */
export function formatPrecip(mm: number): string {
  return `${mm.toFixed(1)} mm`;
}

/** Format a duration in hours to a human-readable string. */
export function formatDuration(hours: number): string {
  const totalMinutes = Math.round(hours * 60);
  const h = Math.floor(totalMinutes / 60);
  const m = totalMinutes % 60;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** Format an ISO date string to local time (HH:MM). */
export function formatTime(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZone: RACE_TIMEZONE,
  });
}

/** Format an ISO date string to local time with timezone (HH:MM CET). */
export function formatTimeWithZone(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
    timeZone: RACE_TIMEZONE,
  });
}

/** Convert wind direction in degrees to compass label. Normalizes negative and >360 values. */
export function windDirectionLabel(degrees: number): string {
  const directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
  // Normalize to 0–360 range (handles negative and >360 values)
  const normalized = ((degrees % 360) + 360) % 360;
  const index = Math.round(normalized / 45) % 8;
  return directions[index] ?? "N";
}

/** Format an ISO date string to a human-readable date (e.g. "Sun 1 Mar 2026"). */
export function formatDate(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleDateString("en-GB", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: RACE_TIMEZONE,
  });
}

/** Format cloud cover as percentage. */
export function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}
