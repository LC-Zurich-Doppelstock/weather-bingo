import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import CourseOverview from "./CourseOverview";
import type { Checkpoint, RaceForecastResponse } from "../../api/types";

// Mock recharts to avoid canvas rendering issues in jsdom
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

const mockCheckpoints: Checkpoint[] = [
  {
    id: "cp-1",
    name: "Salen",
    distance_km: 0,
    latitude: 61.16,
    longitude: 13.27,
    elevation_m: 400,
    sort_order: 1,
  },
  {
    id: "cp-2",
    name: "Mangsbodarna",
    distance_km: 24,
    latitude: 61.12,
    longitude: 13.68,
    elevation_m: 520,
    sort_order: 2,
  },
];

const mockRaceForecast: RaceForecastResponse = {
  race_id: "race-1",
  race_name: "Vasaloppet",
  target_duration_hours: 8,
  yr_model_run_at: "2026-02-28T06:00:00Z",
  forecast_horizon: "2026-03-09T12:00:00Z",
  checkpoints: [
    {
      checkpoint_id: "cp-1",
      name: "Salen",
      distance_km: 0,
      expected_time: "2026-03-01T07:00:00Z",
      forecast_available: true,
      weather: {
        temperature_c: -5,
        temperature_percentile_10_c: -8,
        temperature_percentile_90_c: -2,
        feels_like_c: -10,
        snow_temperature_c: -7.3,
        wind_speed_ms: 3.2,
        wind_speed_percentile_10_ms: 1.5,
        wind_speed_percentile_90_ms: 5.0,
        wind_direction_deg: 180,
        precipitation_mm: 0.5,
        precipitation_type: "snow",
        symbol_code: "heavysnow",
      },
    },
    {
      checkpoint_id: "cp-2",
      name: "Mangsbodarna",
      distance_km: 24,
      expected_time: "2026-03-01T09:08:00Z",
      forecast_available: true,
      weather: {
        temperature_c: -3,
        temperature_percentile_10_c: -6,
        temperature_percentile_90_c: -1,
        feels_like_c: -7,
        snow_temperature_c: -4.8,
        wind_speed_ms: 2.5,
        wind_speed_percentile_10_ms: 1.0,
        wind_speed_percentile_90_ms: 4.0,
        wind_direction_deg: 225,
        precipitation_mm: 0.2,
        precipitation_type: "snow",
        symbol_code: "lightsnow",
      },
    },
  ],
};

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
    const allUnavailable: RaceForecastResponse = {
      ...mockRaceForecast,
      forecast_horizon: "2026-02-25T18:00:00Z",
      checkpoints: mockRaceForecast.checkpoints.map((cp) => ({
        ...cp,
        forecast_available: false,
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
    const partial: RaceForecastResponse = {
      ...mockRaceForecast,
      checkpoints: [
        mockRaceForecast.checkpoints[0]!,
        {
          ...mockRaceForecast.checkpoints[1]!,
          forecast_available: false,
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
    const emptyForecast: RaceForecastResponse = {
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
