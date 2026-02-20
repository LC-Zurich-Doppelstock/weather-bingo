import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import ForecastHistory from "./ForecastHistory";
import { createWrapper, setupMswLifecycle } from "../../test/helpers";
import type { ForecastHistoryResponse } from "../../api/types";

// --- Recharts mock (must be self-contained, no imported variables) ---
vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  ComposedChart: ({
    children,
    data,
  }: {
    children: React.ReactNode;
    data?: unknown[];
  }) => (
    <div data-testid="ComposedChart" data-points={data?.length ?? 0}>
      {children}
    </div>
  ),
  BarChart: ({
    children,
    data,
  }: {
    children: React.ReactNode;
    data?: unknown[];
  }) => (
    <div data-testid="BarChart" data-points={data?.length ?? 0}>
      {children}
    </div>
  ),
  Line: () => null,
  Area: () => null,
  Bar: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
}));

/** Factory for a mock history response with N model runs. */
function createMockHistoryResponse(
  numEntries: number,
  overrides?: Partial<ForecastHistoryResponse>
): ForecastHistoryResponse {
  const history = Array.from({ length: numEntries }, (_, i) => {
    const runDate = new Date("2026-02-19T06:00:00Z");
    runDate.setHours(runDate.getHours() + i * 6);
    return {
      fetched_at: new Date(runDate.getTime() + 60_000).toISOString(),
      yr_model_run_at: runDate.toISOString(),
      model_run_at: runDate.toISOString(),
      weather: {
        temperature_c: -5 + i * 0.3,
        temperature_percentile_10_c: -8 + i * 0.3,
        temperature_percentile_90_c: -2 + i * 0.3,
        feels_like_c: -10 + i * 0.3,
        snow_temperature_c: -7 + i * 0.2,
        wind_speed_ms: 3.2 + i * 0.1,
        wind_speed_percentile_10_ms: 1.5,
        wind_speed_percentile_90_ms: 5.0,
        wind_direction_deg: 180,
        wind_gust_ms: 6.1,
        precipitation_mm: 0.5 + i * 0.1,
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
  });
  return {
    checkpoint_id: "cp-1",
    checkpoint_name: "Salen",
    forecast_time: "2026-02-20T08:00:00Z",
    history,
    ...overrides,
  };
}

const mockHistoryResponse = createMockHistoryResponse(5);

const server = setupServer(
  http.get(
    "/api/v1/forecasts/checkpoint/:checkpointId/history",
    () => HttpResponse.json(mockHistoryResponse)
  )
);

setupMswLifecycle(server);

describe("ForecastHistory", () => {
  it("renders collapsed by default", () => {
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );
    const button = screen.getByRole("button", { name: /forecast history/i });
    expect(button).toBeInTheDocument();
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("does not fetch data when collapsed", () => {
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );
    // Charts should not be visible
    expect(screen.queryByTestId("ComposedChart")).not.toBeInTheDocument();
    expect(screen.queryByTestId("BarChart")).not.toBeInTheDocument();
  });

  it("expands and loads history charts on click", async () => {
    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    const button = screen.getByRole("button", { name: /forecast history/i });
    await user.click(button);

    expect(button).toHaveAttribute("aria-expanded", "true");

    // Wait for charts to appear
    await waitFor(() => {
      expect(screen.getAllByTestId("ComposedChart")).toHaveLength(2); // temp + wind
    });
    expect(screen.getByTestId("BarChart")).toBeInTheDocument(); // precipitation
  });

  it("shows model run count after loading", async () => {
    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(screen.getByText("5 model runs")).toBeInTheDocument();
    });
  });

  it("shows singular 'model run' for single entry", async () => {
    const singleResponse = createMockHistoryResponse(1);
    server.use(
      http.get(
        "/api/v1/forecasts/checkpoint/:checkpointId/history",
        () => HttpResponse.json(singleResponse)
      )
    );

    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(screen.getByText("1 model run")).toBeInTheDocument();
    });
  });

  it("shows empty message when history has no entries", async () => {
    const emptyResponse = createMockHistoryResponse(0);
    server.use(
      http.get(
        "/api/v1/forecasts/checkpoint/:checkpointId/history",
        () => HttpResponse.json(emptyResponse)
      )
    );

    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(screen.getByText("No history data available yet.")).toBeInTheDocument();
    });
  });

  it("shows error message on API failure", async () => {
    server.use(
      http.get(
        "/api/v1/forecasts/checkpoint/:checkpointId/history",
        () => new HttpResponse(null, { status: 500 })
      )
    );

    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(screen.getByText("Failed to load forecast history.")).toBeInTheDocument();
    });
  });

  it("renders chart section labels", async () => {
    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("img", { name: "Temperature history chart" })
      ).toBeInTheDocument();
    });
    expect(
      screen.getByRole("img", { name: "Precipitation history chart" })
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Wind Speed history chart" })
    ).toBeInTheDocument();
  });

  it("collapses when clicked again", async () => {
    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    const button = screen.getByRole("button", { name: /forecast history/i });

    // Expand
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");

    // Collapse
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("passes data points to charts", async () => {
    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      const composedCharts = screen.getAllByTestId("ComposedChart");
      expect(composedCharts[0]).toHaveAttribute("data-points", "5");
    });

    const barChart = screen.getByTestId("BarChart");
    expect(barChart).toHaveAttribute("data-points", "5");
  });

  it("handles null yr_model_run_at (pre-poller rows)", async () => {
    const legacyResponse: ForecastHistoryResponse = {
      ...createMockHistoryResponse(2),
      history: [
        {
          fetched_at: "2026-02-19T06:26:00Z",
          yr_model_run_at: null,
          model_run_at: "2026-02-19T06:26:00Z", // fallback to fetched_at
          weather: createMockHistoryResponse(1).history[0]!.weather,
        },
        {
          fetched_at: "2026-02-19T13:00:00Z",
          yr_model_run_at: "2026-02-19T12:00:00Z",
          model_run_at: "2026-02-19T12:00:00Z",
          weather: createMockHistoryResponse(1).history[0]!.weather,
        },
      ],
    };
    server.use(
      http.get(
        "/api/v1/forecasts/checkpoint/:checkpointId/history",
        () => HttpResponse.json(legacyResponse)
      )
    );

    const user = userEvent.setup();
    render(
      <ForecastHistory checkpointId="cp-1" passTime="2026-02-20T08:00:00Z" />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: /forecast history/i }));

    await waitFor(() => {
      expect(screen.getByText("2 model runs")).toBeInTheDocument();
    });
  });
});
