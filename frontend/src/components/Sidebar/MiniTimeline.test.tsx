import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import MiniTimeline from "./MiniTimeline";
import { createWrapper, setupMswLifecycle } from "../../test/helpers";
import { createMockForecast } from "../../test/fixtures";

const server = setupServer(
  http.get("/api/v1/forecasts/checkpoint/:checkpointId", ({ request }) => {
    const url = new URL(request.url);
    const datetime = url.searchParams.get("datetime") ?? "";
    // Return different temperatures for different time slots
    const date = new Date(datetime);
    const hour = date.getUTCHours();
    return HttpResponse.json(createMockForecast(datetime, -5 + hour * 0.5));
  })
);

setupMswLifecycle(server);

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
