import { describe, it, expect } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import CheckpointDetail from "./CheckpointDetail";
import { createWrapper, setupMswLifecycle } from "../../test/helpers";
import type { ForecastResponse } from "../../api/types";
import {
  mockSalenCheckpoint,
  mockForecastResponse,
} from "../../test/fixtures";

// MSW server to handle MiniTimeline's forecast requests
const server = setupServer(
  http.get("/api/v1/forecasts/checkpoint/:checkpointId", () => {
    return HttpResponse.json(mockForecastResponse);
  })
);

setupMswLifecycle(server);

describe("CheckpointDetail", () => {
  it("renders loading skeleton", () => {
    const { container } = render(
      <CheckpointDetail
        checkpoint={mockSalenCheckpoint}
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
        checkpoint={mockSalenCheckpoint}
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
      ...mockForecastResponse,
      forecast_available: false,
      weather: null,
      fetched_at: null,
      source: null,
      forecast_horizon: "2026-02-25T18:00:00Z",
    };
    render(
      <CheckpointDetail
        checkpoint={mockSalenCheckpoint}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
        isLoading={false}
      />,
      { wrapper: createWrapper() }
    );
    const snowGroup = screen.getByRole("group", { name: "Snow Temperature" });
    expect(snowGroup).toBeInTheDocument();
    expect(within(snowGroup).getByText("-7°C")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Snow temperature info" })).toBeInTheDocument();
  });

  it("renders wind data with direction", () => {
    render(
      <CheckpointDetail
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("0.5")).toBeInTheDocument();
  });

  it("shows stale data badge when forecast is stale", () => {
    const staleForecast = { ...mockForecastResponse, stale: true };
    render(
      <CheckpointDetail
        checkpoint={mockSalenCheckpoint}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows source and model run metadata", () => {
    render(
      <CheckpointDetail
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
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
        checkpoint={mockSalenCheckpoint}
        passTime="2026-03-01T07:00:00Z"
        forecast={mockForecastResponse}
        isLoading={false}

      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByRole("group", { name: "Temperature" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Wind" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Precipitation" })).toBeInTheDocument();
  });
});
