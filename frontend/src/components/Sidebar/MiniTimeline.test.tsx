import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
  it("renders collapsible header with title", () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    expect(screen.getByText("When Paced Differently")).toBeInTheDocument();
  });

  it("starts collapsed (no chart visible)", () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // Header button should have aria-expanded=false
    const button = screen.getByRole("button", { name: "When Paced Differently" });
    expect(button).toHaveAttribute("aria-expanded", "false");

    // Chart should not be visible (panel has max-h-0)
    const panel = document.getElementById("mini-timeline-panel");
    expect(panel).toHaveClass("max-h-0");
  });

  it("expands on click and shows loading state", async () => {
    const user = userEvent.setup();

    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    const button = screen.getByRole("button", { name: "When Paced Differently" });
    await user.click(button);

    expect(button).toHaveAttribute("aria-expanded", "true");
    const panel = document.getElementById("mini-timeline-panel");
    expect(panel).not.toHaveClass("max-h-0");
  });

  it("renders chart region with accessible label after expanding", async () => {
    const user = userEvent.setup();

    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // Expand the section
    await user.click(screen.getByRole("button", { name: "When Paced Differently" }));

    expect(
      await screen.findByRole("img", { name: "Weather at different paces chart" })
    ).toBeInTheDocument();
  });

  it("does not fetch data when collapsed", () => {
    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // When collapsed, no chart should be rendered (lazy loading)
    expect(screen.queryByRole("img", { name: "Weather at different paces chart" })).toBeNull();
  });

  it("collapses again on second click", async () => {
    const user = userEvent.setup();

    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    const button = screen.getByRole("button", { name: "When Paced Differently" });

    // Expand
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");

    // Collapse
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "false");
    const panel = document.getElementById("mini-timeline-panel");
    expect(panel).toHaveClass("max-h-0");
  });

  it("shows legend text after expanding and data loads", async () => {
    const user = userEvent.setup();

    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    await user.click(screen.getByRole("button", { name: "When Paced Differently" }));

    expect(await screen.findByText("Dashed line = pass time")).toBeInTheDocument();
  });

  it("shows no chart when all forecasts have null weather", async () => {
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

    const user = userEvent.setup();

    render(
      <MiniTimeline
        checkpointId="cp-1"
        passTime="2026-03-01T09:00:00Z"
      />,
      { wrapper: createWrapper() }
    );

    // Expand to trigger fetching
    await user.click(screen.getByRole("button", { name: "When Paced Differently" }));

    // Wait for data to settle — no chart should render
    await waitFor(() => {
      expect(screen.queryByRole("img", { name: "Weather at different paces chart" })).toBeNull();
    });
  });
});
