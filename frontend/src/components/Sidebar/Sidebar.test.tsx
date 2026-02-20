import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import Sidebar from "./Sidebar";
import { createWrapper, setupMswLifecycle } from "../../test/helpers";
import {
  mockVasaloppetRace,
  mockCheckpoints,
  mockRaceForecast,
  mockForecastResponse,
} from "../../test/fixtures";

const server = setupServer(
  http.get("/api/v1/forecasts/race/:raceId", () => {
    return HttpResponse.json(mockRaceForecast);
  }),
  http.get("/api/v1/forecasts/checkpoint/:checkpointId", () => {
    return HttpResponse.json(mockForecastResponse);
  })
);

setupMswLifecycle(server);

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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
        race={mockVasaloppetRace}
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
