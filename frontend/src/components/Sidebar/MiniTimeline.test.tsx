import { describe, it, expect, beforeAll, afterEach, afterAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import MiniTimeline from "./MiniTimeline";

import type { ForecastResponse } from "../../api/types";

const mockForecast = (time: string, tempC: number): ForecastResponse => ({
  checkpoint_id: "cp-1",
  checkpoint_name: "Salen",
  forecast_time: time,
  forecast_available: true,
  fetched_at: "2026-02-28T12:00:00Z",
  yr_model_run_at: "2026-02-28T06:00:00Z",
  source: "yr.no",
  stale: false,
  forecast_horizon: "2026-03-09T12:00:00Z",
  weather: {
    temperature_c: tempC,
    temperature_percentile_10_c: tempC - 3,
    temperature_percentile_90_c: tempC + 2,
    feels_like_c: tempC - 5,
    snow_temperature_c: Math.min(tempC - 1, 0),
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
});

const server = setupServer(
  http.get("/api/v1/forecasts/checkpoint/:checkpointId", ({ request }) => {
    const url = new URL(request.url);
    const datetime = url.searchParams.get("datetime") ?? "";
    // Return different temperatures for different time slots
    const date = new Date(datetime);
    const hour = date.getUTCHours();
    return HttpResponse.json(mockForecast(datetime, -5 + hour * 0.5));
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

describe("MiniTimeline", () => {
  it("renders timeline header", async () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    expect(await screen.findByText("Timeline")).toBeInTheDocument();
  });

  it("shows loading state initially", () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // Should show timeline label even while loading
    expect(screen.getByText("Timeline")).toBeInTheDocument();
  });

  it("renders chart region with accessible label", async () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    expect(
      await screen.findByRole("img", { name: "Weather timeline chart" })
    ).toBeInTheDocument();
  });

  it("returns null when no data is available", async () => {
    server.use(
      http.get("/api/v1/forecasts/checkpoint/:checkpointId", () => {
        return HttpResponse.json({
          checkpoint_id: "cp-1",
          checkpoint_name: "Salen",
          forecast_time: "2026-03-01T09:00:00Z",
          forecast_available: false,
          fetched_at: null,
          yr_model_run_at: null,
          source: null,
          stale: false,
          weather: null,
        });
      })
    );

    const { container } = render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // Wait for queries to resolve — the loading skeleton disappears,
    // and since there's no temperature data, the component returns null
    await screen.findByText("Timeline"); // loading state shows this
    // Wait for data to settle — no chart should render
    await waitFor(() => {
      expect(container.querySelector("[role='img']")).toBeNull();
    });
  });

  it("shows legend text", async () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    expect(await screen.findByText("Dashed line = pass time")).toBeInTheDocument();
  });
});
