/**
 * Weather Bingo colour palette.
 * Derived from a tropical botanical artwork on black background.
 * See specs.md Section 5.6 for full reference.
 */

// UI Colours
export const colors = {
  background: "#0D0D0C",
  surface: "#171614",
  surfaceAlt: "#1F1E1C",
  primary: "#2DD4A8",
  primaryHover: "#34EBB9",
  secondary: "#14B8A6",
  accentWarm: "#F5A623",
  accentCool: "#7C8CF5",
  accentRose: "#D4687A",
  textPrimary: "#F0EEEB",
  textSecondary: "#9E9A93",
  textMuted: "#8A8580",
  border: "#2C2A27",
  error: "#EF4444",
  success: "#2DD4A8",
} as const;

// Chart colours (ordered for data visualisation, per specs.md §5.6)
export const chartColors = {
  temperature: "#2DD4A8",
  feelsLike: "#14B8A6",
  wind: "#F5A623",
  precipitation: "#7C8CF5",
  humidity: "#34EBB9",
  cloudCover: "#5A7A6E",
  elevation: "#D4687A",
  snowTemperature: "#88C8E8",
} as const;

// Uncertainty bands use same colour at 15% opacity
export const uncertaintyOpacity = 0.15;

/**
 * Opacity for secondary line overlays (e.g. "feels like" line on temperature chart).
 * Visually lighter than the primary data line but clearly visible.
 */
export const secondaryLineOpacity = 0.6;
