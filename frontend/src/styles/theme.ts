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
  textPrimary: "#F0EEEB",
  textSecondary: "#9E9A93",
  textMuted: "#6B6762",
  border: "#2C2A27",
  error: "#EF4444",
  success: "#2DD4A8",
} as const;

// Chart colours (ordered for data visualisation)
export const chartColors = {
  temperature: "#2DD4A8",
  feelsLike: "#14B8A6",
  wind: "#7C8CF5",
  precipitation: "#F5A623",
  humidity: "#34EBB9",
  cloudCover: "#5A7A6E",
} as const;

// Uncertainty bands use same colour at 15% opacity
export const uncertaintyOpacity = 0.15;
