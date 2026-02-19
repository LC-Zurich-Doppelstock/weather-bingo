import { describe, it, expect, beforeAll, afterEach, afterAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import type { Checkpoint, ForecastResponse } from "../../api/types";
import CheckpointDetail from "./CheckpointDetail";

const mockCheckpoint: Checkpoint = {
  id: "cp-1",
  name: "Salen",
  distance_km: 0,
  latitude: 61.16,
  longitude: 13.27,
  elevation_m: 400,
  sort_order: 1,
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
    temperature_c: -5.0,
    temperature_percentile_10_c: -8.0,
    temperature_percentile_90_c: -2.0,
    feels_like_c: -10.5,
    snow_temperature_c: -8.5,
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
    dew_point_c: -7.0,
    cloud_cover_pct: 90,
    uv_index: 0.5,
    symbol_code: "heavysnow",
  },
};

// MSW server to handle MiniTimeline's forecast requests
const server = setupServer(
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

describe("CheckpointDetail", () => {
  it("renders loading skeleton", () => {
    const { container } = render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={null}
        isLoading={true}

      />,
      { wrapper: createWrapper() }
    );
    expect(container.querySelector("[aria-busy='true']")).toBeInTheDocument();
    expect(screen.getByText("Salen")).toBeInTheDocument();
  });

  it("renders no-data message when forecast is null and not loading", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={null}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(
      screen.getByText("No forecast data available for this time.")
    ).toBeInTheDocument();
  });

  it("renders forecast-unavailable message when weather is null", () => {
    const unavailableForecast: ForecastResponse = {
      ...mockForecast,
      forecast_available: false,
      weather: null,
      fetched_at: null,
      source: null,
      forecast_horizon: "2026-02-25T18:00:00Z",
    };
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={unavailableForecast}
        isLoading={false}
      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Salen")).toBeInTheDocument();
    expect(
      screen.getByText(/forecast not yet available/i)
    ).toBeInTheDocument();
    // Should show the dynamic horizon date
    expect(
      screen.getByText(/forecast horizon/i)
    ).toBeInTheDocument();
  });

  it("renders checkpoint name and distance", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Salen")).toBeInTheDocument();
    expect(screen.getByText("(0.0 km)")).toBeInTheDocument();
  });

  it("renders temperature data", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("-5°C")).toBeInTheDocument();
    expect(screen.getByText(/feels like -10°C/i)).toBeInTheDocument();
  });

  it("renders snow temperature with info popover button", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}
      />,
      { wrapper: createWrapper() }
    );
    const snowGroup = screen.getByRole("group", { name: "Snow Temperature" });
    expect(snowGroup).toBeInTheDocument();
    expect(screen.getByText("-8°C")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Snow temperature info" })).toBeInTheDocument();
  });

  it("renders wind data with direction", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("3.2 m/s")).toBeInTheDocument();
    expect(screen.getByText("S")).toBeInTheDocument();
  });

  it("renders precipitation type and amount", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    const precipGroup = screen.getByRole("group", { name: "Precipitation" });
    expect(within(precipGroup).getByText(/snow/i)).toBeInTheDocument();
    expect(screen.getByText("0.5 mm")).toBeInTheDocument();
  });

  it("renders humidity and cloud cover", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("85%")).toBeInTheDocument();
    expect(screen.getByText("90%")).toBeInTheDocument();
  });

  it("renders UV index when available", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("0.5")).toBeInTheDocument();
  });

  it("shows stale data badge when forecast is stale", () => {
    const staleForecast = { ...mockForecast, stale: true };
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={staleForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText("Forecast data may be outdated (yr.no unavailable)")
    ).toBeInTheDocument();
  });

  it("does not show stale badge for fresh data", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows source and model run metadata", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Source: yr.no")).toBeInTheDocument();
    expect(screen.getByText(/Model run:/)).toBeInTheDocument();
  });

  it("renders weather rows with aria group labels", () => {
    render(
      <CheckpointDetail
        checkpoint={mockCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecast}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByRole("group", { name: "Temperature" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Wind" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Precipitation" })).toBeInTheDocument();
  });
});
