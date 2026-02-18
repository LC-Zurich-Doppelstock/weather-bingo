import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ElevationProfile from "./ElevationProfile";
import type { CoursePoint, Checkpoint } from "../../api/types";

// Mock recharts to avoid canvas rendering issues in jsdom
vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  AreaChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="area-chart">{children}</div>
  ),
  Area: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  ReferenceLine: () => null,
}));

const mockCourse: CoursePoint[] = [
  { lat: 61.16, lon: 13.27, ele: 400 },
  { lat: 61.15, lon: 13.30, ele: 420 },
  { lat: 61.14, lon: 13.35, ele: 380 },
  { lat: 61.12, lon: 13.40, ele: 520 },
  { lat: 61.10, lon: 13.50, ele: 450 },
];

const mockCheckpoints: Checkpoint[] = [
  {
    id: "cp-1",
    name: "Salen Start",
    distance_km: 0,
    latitude: 61.16,
    longitude: 13.27,
    elevation_m: 400,
    sort_order: 1,
  },
  {
    id: "cp-2",
    name: "Mangsbodarna",
    distance_km: 12,
    latitude: 61.12,
    longitude: 13.40,
    elevation_m: 520,
    sort_order: 2,
  },
];

describe("ElevationProfile", () => {
  it("renders nothing when course is null", () => {
    const { container } = render(
      <ElevationProfile
        course={null}
        checkpoints={mockCheckpoints}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders nothing when course is empty", () => {
    const { container } = render(
      <ElevationProfile
        course={[]}
        checkpoints={mockCheckpoints}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders the elevation profile with header and chart", () => {
    render(
      <ElevationProfile
        course={mockCourse}
        checkpoints={mockCheckpoints}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );
    expect(screen.getByText("Elevation Profile")).toBeInTheDocument();
    expect(screen.getByTestId("area-chart")).toBeInTheDocument();
    expect(screen.getByTestId("elevation-profile")).toBeInTheDocument();
  });

  it("has hidden lg:block class for desktop-only display", () => {
    render(
      <ElevationProfile
        course={mockCourse}
        checkpoints={mockCheckpoints}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );
    const wrapper = screen.getByTestId("elevation-profile");
    expect(wrapper.className).toContain("hidden");
    expect(wrapper.className).toContain("lg:block");
  });

  it("collapses and expands when toggle button is clicked", () => {
    render(
      <ElevationProfile
        course={mockCourse}
        checkpoints={mockCheckpoints}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );

    const toggleButton = screen.getByRole("button", {
      name: /elevation profile/i,
    });
    const content = document.getElementById("elevation-profile-content");
    expect(content).toBeInTheDocument();

    // Initially expanded
    expect(toggleButton).toHaveAttribute("aria-expanded", "true");
    expect(content!.className).toContain("max-h-[200px]");

    // Collapse
    fireEvent.click(toggleButton);
    expect(toggleButton).toHaveAttribute("aria-expanded", "false");
    expect(content!.className).toContain("max-h-0");

    // Expand again
    fireEvent.click(toggleButton);
    expect(toggleButton).toHaveAttribute("aria-expanded", "true");
    expect(content!.className).toContain("max-h-[200px]");
  });

  it("renders with empty checkpoints array", () => {
    render(
      <ElevationProfile
        course={mockCourse}
        checkpoints={[]}
        hoveredCheckpointId={null}
        selectedCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />,
    );
    // Should still render the profile (just without checkpoint markers)
    expect(screen.getByTestId("area-chart")).toBeInTheDocument();
  });
});
