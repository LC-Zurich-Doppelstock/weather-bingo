/**
 * Shared Recharts style objects for consistent chart appearance.
 *
 * Hoisted as module-level constants so they are never recreated on render.
 * Import these instead of defining identical objects in each chart component.
 */

import { colors } from "./theme";

/** Tooltip container style applied via Recharts `contentStyle` prop. */
export const tooltipStyle = {
  backgroundColor: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: "6px",
  color: colors.textPrimary,
  fontSize: "12px",
} as const;

/** Axis tick label style (font size + muted colour). */
export const tickStyle = { fill: colors.textMuted, fontSize: 10 } as const;

/** Axis line style (border colour). */
export const axisLineStyle = { stroke: colors.border } as const;
