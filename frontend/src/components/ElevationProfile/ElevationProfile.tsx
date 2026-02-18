import { memo, useCallback, useMemo, useState } from "react";
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
} from "recharts";
import type { CategoricalChartState } from "recharts/types/chart/types";
import type { CoursePoint, Checkpoint } from "../../api/types";
import { chartColors, colors, uncertaintyOpacity } from "../../styles/theme";
import { computeElevationProfile } from "../../utils/geo";
import type { ElevationPoint } from "../../utils/geo";

// Static style objects (same pattern as CourseOverview)
const tooltipStyle = {
  backgroundColor: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: "6px",
  color: colors.textPrimary,
  fontSize: "12px",
} as const;

const tickStyle = { fill: colors.textMuted, fontSize: 10 } as const;
const axisLineStyle = { stroke: colors.border } as const;

/** Chevron-down SVG path (rotates when collapsed). */
const ChevronIcon = ({ collapsed }: { collapsed: boolean }) => (
  <svg
    className={`h-4 w-4 text-text-muted transition-transform duration-200 ${
      collapsed ? "-rotate-90" : ""
    }`}
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
  </svg>
);

interface ElevationProfileProps {
  /** Full GPS course track. */
  course: CoursePoint[] | null;
  /** Race checkpoints with distance_km for marker placement. */
  checkpoints: Checkpoint[];
  /** Currently hovered checkpoint ID (bidirectional sync). */
  hoveredCheckpointId: string | null;
  /** Currently selected checkpoint ID. */
  selectedCheckpointId: string | null;
  /** Callback when a checkpoint is hovered via the elevation chart. */
  onCheckpointHover: (id: string | null) => void;
}

/**
 * Elevation profile chart showing altitude along the race course.
 * Positioned below the map, collapsible, desktop-only (hidden on mobile).
 *
 * Uses the full GPS track for a smooth profile line, with checkpoint
 * markers shown as vertical dashed reference lines.
 */
const ElevationProfile = memo(function ElevationProfile({
  course,
  checkpoints,
  hoveredCheckpointId,
  selectedCheckpointId,
  onCheckpointHover,
}: ElevationProfileProps) {
  const [collapsed, setCollapsed] = useState(false);

  // Compute elevation profile data from GPS track
  const profileData: ElevationPoint[] = useMemo(() => {
    if (!course || course.length === 0) return [];
    return computeElevationProfile(course);
  }, [course]);

  // Find the distance of the hovered checkpoint for the reference line
  const hoveredDistance = useMemo(() => {
    if (!hoveredCheckpointId) return undefined;
    const cp = checkpoints.find((c) => c.id === hoveredCheckpointId);
    return cp?.distance_km;
  }, [hoveredCheckpointId, checkpoints]);

  // Find the distance of the selected checkpoint for the reference line
  const selectedDistance = useMemo(() => {
    if (!selectedCheckpointId) return undefined;
    const cp = checkpoints.find((c) => c.id === selectedCheckpointId);
    return cp?.distance_km;
  }, [selectedCheckpointId, checkpoints]);

  // Checkpoint distances for x-axis ticks (show only checkpoint positions)
  const checkpointTicks = useMemo(
    () => checkpoints.map((cp) => cp.distance_km),
    [checkpoints],
  );

  // Map checkpoint distance -> short name for x-axis tick labels
  const checkpointNameByDistance = useMemo(() => {
    const map = new Map<number, string>();
    for (const cp of checkpoints) {
      map.set(cp.distance_km, cp.name.split(" ")[0] ?? cp.name);
    }
    return map;
  }, [checkpoints]);

  // When hovering on the chart, find the nearest checkpoint and notify parent
  const handleChartMouseMove = useCallback(
    (state: CategoricalChartState) => {
      if (
        state?.activeTooltipIndex != null &&
        profileData.length > 0 &&
        checkpoints.length > 0
      ) {
        const hoveredKm = profileData[state.activeTooltipIndex]?.distance_km;
        if (hoveredKm == null) return;

        // Find the nearest checkpoint by distance
        let nearestId = checkpoints[0]!.id;
        let minDelta = Math.abs(hoveredKm - checkpoints[0]!.distance_km);
        for (let i = 1; i < checkpoints.length; i++) {
          const cp = checkpoints[i]!;
          const delta = Math.abs(hoveredKm - cp.distance_km);
          if (delta < minDelta) {
            minDelta = delta;
            nearestId = cp.id;
          }
        }
        onCheckpointHover(nearestId);
      }
    },
    [profileData, checkpoints, onCheckpointHover],
  );

  const handleChartMouseLeave = useCallback(() => {
    onCheckpointHover(null);
  }, [onCheckpointHover]);

  const toggleCollapse = useCallback(() => {
    setCollapsed((prev) => !prev);
  }, []);

  // Downsample data for performance — GPS tracks can have thousands of points.
  // Keep at most ~500 points for the chart (enough for smooth rendering).
  const chartData = useMemo(() => {
    if (profileData.length <= 500) return profileData;
    const step = Math.ceil(profileData.length / 500);
    const sampled: ElevationPoint[] = [];
    for (let i = 0; i < profileData.length; i += step) {
      sampled.push(profileData[i]!);
    }
    // Always include the last point
    const last = profileData[profileData.length - 1];
    if (last && sampled[sampled.length - 1] !== last) {
      sampled.push(last);
    }
    return sampled;
  }, [profileData]);

  // Don't render anything if there's no course data
  if (!course || course.length === 0) return null;

  return (
    <div
      className="hidden border-t border-border bg-surface-alt lg:block"
      data-testid="elevation-profile"
    >
      {/* Collapsible header */}
      <button
        type="button"
        onClick={toggleCollapse}
        className="flex w-full items-center justify-between px-3 py-1.5 text-left hover:bg-surface-alt/80"
        aria-expanded={!collapsed}
        aria-controls="elevation-profile-content"
      >
        <span className="text-xs font-medium uppercase tracking-wider text-text-muted">
          Elevation Profile
        </span>
        <ChevronIcon collapsed={collapsed} />
      </button>

      {/* Collapsible content */}
      <div
        id="elevation-profile-content"
        className={`overflow-hidden transition-all duration-200 ${
          collapsed ? "max-h-0" : "max-h-[200px]"
        }`}
      >
        <div className="px-1 pb-1">
          <ResponsiveContainer width="100%" height={140}>
            <AreaChart
              data={chartData}
              margin={{ top: 14, right: 5, bottom: 5, left: 0 }}
              onMouseMove={handleChartMouseMove}
              onMouseLeave={handleChartMouseLeave}
            >
              <XAxis
                dataKey="distance_km"
                type="number"
                domain={["dataMin", "dataMax"]}
                ticks={checkpointTicks}
                tick={tickStyle}
                tickFormatter={(v: number) => `${Math.round(v)} km`}
                axisLine={axisLineStyle}
                tickLine={false}
              />
              <YAxis
                dataKey="ele"
                tick={tickStyle}
                tickFormatter={(v: number) => `${Math.round(v)}`}
                axisLine={false}
                tickLine={false}
                width={45}
                domain={["dataMin - 20", "dataMax + 20"]}
                unit=" m"
              />
              <Tooltip
                contentStyle={tooltipStyle}
                formatter={(value: number) => [
                  `${Math.round(value)} m`,
                  "Elevation",
                ]}
                labelFormatter={(v: number) => `${v.toFixed(1)} km`}
              />

              {/* Checkpoint markers: vertical dashed lines with name labels */}
              {checkpoints.map((cp) => (
                <ReferenceLine
                  key={cp.id}
                  x={cp.distance_km}
                  stroke={colors.textMuted}
                  strokeDasharray="2 4"
                  strokeWidth={0.5}
                  strokeOpacity={0.6}
                  label={{
                    value: checkpointNameByDistance.get(cp.distance_km) ?? cp.name,
                    position: "insideTopRight",
                    fill: colors.textMuted,
                    fontSize: 10,
                    offset: 2,
                  }}
                />
              ))}

              {/* Hovered checkpoint reference line */}
              <ReferenceLine
                x={hoveredDistance ?? 0}
                stroke={colors.accentRose}
                strokeDasharray="3 3"
                strokeWidth={1}
                strokeOpacity={hoveredDistance != null ? 1 : 0}
              />

              {/* Selected checkpoint reference line */}
              <ReferenceLine
                x={selectedDistance ?? 0}
                stroke={colors.accentRose}
                strokeWidth={1.5}
                strokeOpacity={selectedDistance != null ? 0.7 : 0}
              />

              {/* Elevation area fill */}
              <Area
                type="monotone"
                dataKey="ele"
                stroke={chartColors.elevation}
                strokeWidth={1.5}
                fill={chartColors.elevation}
                fillOpacity={uncertaintyOpacity}
                dot={false}
                activeDot={false}
                name="Elevation"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
});

export default ElevationProfile;
