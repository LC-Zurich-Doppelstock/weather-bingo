/**
 * Weather Bingo colour palette.
 * Derived from a tropical botanical artwork on black background.
 * See specs.md Section 5.6 for full reference.
 */

// UI Colours
export const colors = {
  background: "#0A0F0D",
  surface: "#141E1B",
  surfaceAlt: "#1C2B27",
  primary: "#2DD4A8",
  primaryHover: "#34EBB9",
  secondary: "#14B8A6",
  accentWarm: "#F5A623",
  accentCool: "#7C8CF5",
  textPrimary: "#F0F7F4",
  textSecondary: "#8BA89E",
  textMuted: "#5A7A6E",
  border: "#2A3F38",
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
