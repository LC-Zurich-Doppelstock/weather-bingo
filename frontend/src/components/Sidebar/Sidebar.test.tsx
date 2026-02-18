import { describe, it, expect, vi, beforeAll, afterEach, afterAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import Sidebar from "./Sidebar";
import type { Checkpoint, Race, RaceForecastResponse, ForecastResponse } from "../../api/types";

const mockRace: Race = {
  id: "race-1",
  name: "Vasaloppet",
  year: 2026,
  start_time: "2026-03-01T07:00:00Z",
  distance_km: 90,
};

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

const mockForecast: ForecastResponse = {
  checkpoint_id: "cp-1",
  checkpoint_name: "Salen",
  forecast_time: "2026-03-01T07:00:00Z",
  forecast_available: true,
  fetched_at: "2026-02-28T12:00:00Z",
  yr_model_run_at: "2026-02-28T06:00:00Z",
  source: "yr.no",
  stale: false,
  forecast_horizon: "2026-03-09T12:00:00Z",
  weather: {
    temperature_c: -5,
    temperature_percentile_10_c: -8,
    temperature_percentile_90_c: -2,
    feels_like_c: -10,
    wind_speed_ms: 3.2,
    wind_speed_percentile_10_ms: 1.5,
    wind_speed_percentile_90_ms: 5.0,
    wind_direction_deg: 180,
    wind_gust_ms: 6.1,
    precipitation_mm: 0.5,
    precipitation_min_mm: 0.0,
    precipitation_max_mm: 1.2,
    precipitation_type: "snow",
    humidity_pct: 85,
    dew_point_c: -7,
    cloud_cover_pct: 90,
    uv_index: 0.5,
    symbol_code: "heavysnow",
  },
};

const server = setupServer(
  http.get("/api/v1/forecasts/race/:raceId", () => {
    return HttpResponse.json(mockRaceForecast);
  }),
  http.get("/api/v1/forecasts/checkpoint/:checkpointId", () => {
    return HttpResponse.json(mockForecast);
  })
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

describe("Sidebar", () => {
  it("shows message when no race is selected", () => {
    render(
      <Sidebar
        race={null}
        checkpoints={[]}
        selectedCheckpointId={null}
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );
    expect(
      screen.getByText("Select a race to view weather data")
    ).toBeInTheDocument();
  });

  it("shows course overview when race is selected but no checkpoint", async () => {
    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId={null}
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );
    expect(
      await screen.findByText("Weather Along the Course")
    ).toBeInTheDocument();
  });

  it("shows checkpoint detail when a checkpoint is selected", async () => {
    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId="cp-1"
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );
    // Should show the checkpoint name
    expect(await screen.findByText("Salen")).toBeInTheDocument();
    // Should show back button
    expect(screen.getByLabelText("Back to course overview")).toBeInTheDocument();
  });

  it("calls onClearSelection when back button is clicked", async () => {
    const onClearSelection = vi.fn();
    const user = userEvent.setup();

    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId="cp-1"
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={onClearSelection}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );

    const backButton = await screen.findByLabelText("Back to course overview");
    await user.click(backButton);
    expect(onClearSelection).toHaveBeenCalledOnce();
  });

  it("has accessible region labels", async () => {
    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId="cp-1"
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );
    expect(
      await screen.findByRole("region", { name: /Weather details for Salen/ })
    ).toBeInTheDocument();
  });

  it("shows retry button when race forecast fails on course overview", async () => {
    server.use(
      http.get("/api/v1/forecasts/race/:raceId", () => {
        return HttpResponse.json(
          { error: "Internal server error" },
          { status: 500 }
        );
      })
    );

    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId={null}
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );

    expect(
      await screen.findByText("Failed to load forecast data.")
    ).toBeInTheDocument();
    expect(screen.getByText("Retry")).toBeInTheDocument();
  });

  it("shows retry button when race forecast fails with checkpoint selected", async () => {
    server.use(
      http.get("/api/v1/forecasts/race/:raceId", () => {
        return HttpResponse.json(
          { error: "Internal server error" },
          { status: 500 }
        );
      })
    );

    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId="cp-1"
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );

    expect(
      await screen.findByText("Failed to load forecast data.")
    ).toBeInTheDocument();
    expect(screen.getByText("Retry")).toBeInTheDocument();
    // Back button should still be present
    expect(screen.getByLabelText("Back to course overview")).toBeInTheDocument();
  });

  it("shows retry button when individual forecast fails", async () => {
    server.use(
      http.get("/api/v1/forecasts/checkpoint/:checkpointId", () => {
        return HttpResponse.json(
          { error: "Internal server error" },
          { status: 500 }
        );
      })
    );

    render(
      <Sidebar
        race={mockRace}
        checkpoints={mockCheckpoints}
        selectedCheckpointId="cp-1"
        hoveredCheckpointId={null}
        targetDurationHours={8}
        onClearSelection={vi.fn()}
        onCheckpointHover={vi.fn()}
      />,
      { wrapper: createWrapper() }
    );

    expect(
      await screen.findByText("Failed to load forecast data.")
    ).toBeInTheDocument();
    expect(screen.getByText("Retry")).toBeInTheDocument();
  });
});
