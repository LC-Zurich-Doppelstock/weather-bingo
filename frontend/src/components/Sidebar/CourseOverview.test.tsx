import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import CourseOverview from "./CourseOverview";
import {
  mockCheckpoints,
  mockRaceForecast,
} from "../../test/fixtures";

// Mock recharts to avoid canvas rendering issues in jsdom.
// Factory must be self-contained (vi.mock is hoisted above imports).
vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  ComposedChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="composed-chart">{children}</div>
  ),
  BarChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="bar-chart">{children}</div>
  ),
  Line: () => null,
  Area: () => null,
  Bar: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  ReferenceLine: () => null,
}));

describe("CourseOverview", () => {
  it("renders loading skeletons when data is loading", () => {
    const { container } = render(
      <CourseOverview
        raceForecast={null}
        checkpoints={mockCheckpoints}
        isLoading={true}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(screen.getByText("Weather Along the Course")).toBeInTheDocument();
    expect(container.querySelector("[aria-busy='true']")).toBeInTheDocument();
    expect(container.querySelectorAll(".animate-pulse")).toHaveLength(3);
  });

  it("renders forecast-unavailable message when all checkpoints have no weather", () => {
    const allUnavailable = {
      ...mockRaceForecast,
      forecast_horizon: "2026-02-25T18:00:00Z",
      checkpoints: mockRaceForecast.checkpoints.map((cp) => ({
        ...cp,
        forecast_available: false as const,
        weather: null,
      })),
    };

    render(
      <CourseOverview
        raceForecast={allUnavailable}
        checkpoints={mockCheckpoints}
        isLoading={false}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(
      screen.getByText(/forecast not yet available/i)
    ).toBeInTheDocument();
    // Should show the dynamic horizon date
    expect(
      screen.getByText(/forecast horizon/i)
    ).toBeInTheDocument();
  });

  it("renders charts when data is present", () => {
    render(
      <CourseOverview
        raceForecast={mockRaceForecast}
        checkpoints={mockCheckpoints}
        isLoading={false}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(screen.getByText("Weather Along the Course")).toBeInTheDocument();
    expect(screen.getByText(/Vasaloppet/)).toBeInTheDocument();
    expect(screen.getByText(/8h target/)).toBeInTheDocument();
    // Three chart sections
    expect(screen.getByRole("img", { name: "Temperature chart" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Precipitation chart" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Wind Speed chart" })).toBeInTheDocument();
  });

  it("renders partial data warning when some checkpoints have unavailable forecasts", () => {
    const partial = {
      ...mockRaceForecast,
      checkpoints: [
        mockRaceForecast.checkpoints[0]!,
        {
          ...mockRaceForecast.checkpoints[1]!,
          forecast_available: false as const,
          weather: null,
        },
      ],
    };

    render(
      <CourseOverview
        raceForecast={partial}
        checkpoints={mockCheckpoints}
        isLoading={false}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(
      screen.getByText(/some checkpoints are beyond the forecast horizon/i)
    ).toBeInTheDocument();
  });

  it("returns null when not loading and no race forecast", () => {
    const { container } = render(
      <CourseOverview
        raceForecast={null}
        checkpoints={mockCheckpoints}
        isLoading={false}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(container.innerHTML).toBe("");
  });

  it("shows no checkpoints message when checkpoints array is empty", () => {
    const emptyForecast = {
      ...mockRaceForecast,
      checkpoints: [],
    };

    render(
      <CourseOverview
        raceForecast={emptyForecast}
        checkpoints={[]}
        isLoading={false}
        stale={false}
        hoveredCheckpointId={null}
        onCheckpointHover={vi.fn()}
      />
    );
    expect(screen.getByText("No checkpoints available")).toBeInTheDocument();
  });
});
