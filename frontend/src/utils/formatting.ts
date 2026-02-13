/**
 * Formatting utilities for weather data display.
 */

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

/** Format distance in km. */
export function formatDistance(km: number): string {
  return `${Math.round(km)} km`;
}

/** Format a duration in hours to a human-readable string. */
export function formatDuration(hours: number): string {
  const h = Math.floor(hours);
  const m = Math.round((hours - h) * 60);
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** Format an ISO date string to local time (HH:MM). */
export function formatTime(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "Europe/Stockholm",
  });
}

/** Format an ISO date string to local time with timezone (HH:MM CET). */
export function formatTimeWithZone(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
    timeZone: "Europe/Stockholm",
  });
}

/** Convert wind direction in degrees to compass label. */
export function windDirectionLabel(degrees: number): string {
  const directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
  const index = Math.round(degrees / 45) % 8;
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
    timeZone: "Europe/Stockholm",
  });
}

/** Format cloud cover as percentage. */
export function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}
