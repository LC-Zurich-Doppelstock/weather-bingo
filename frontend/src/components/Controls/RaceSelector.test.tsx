import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import RaceSelector from "./RaceSelector";
import { createWrapper, setupMswLifecycle } from "../../test/helpers";
import { mockVasaloppetRace, mockTjejvasanRace } from "../../test/fixtures";
import type { Race } from "../../api/types";

const mockRaces: Race[] = [mockVasaloppetRace, mockTjejvasanRace];

const server = setupServer(
  http.get("/api/v1/races", () => {
    return HttpResponse.json(mockRaces);
  })
);

setupMswLifecycle(server);

describe("RaceSelector", () => {
  it("shows loading state initially", () => {
    render(
      <RaceSelector selectedRaceId={null} onSelect={vi.fn()} />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Loading races...")).toBeInTheDocument();
  });

  it("renders races after loading", async () => {
    render(
      <RaceSelector selectedRaceId={null} onSelect={vi.fn()} />,
      { wrapper: createWrapper() }
    );
    expect(
      await screen.findByText("Vasaloppet 2026")
    ).toBeInTheDocument();
    expect(screen.getByText("Tjejvasan 2026")).toBeInTheDocument();
  });

  it("calls onSelect when a race is chosen", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();

    render(
      <RaceSelector selectedRaceId={null} onSelect={onSelect} />,
      { wrapper: createWrapper() }
    );

    const select = await screen.findByLabelText("Select a race");
    await user.selectOptions(select, "race-2");
    expect(onSelect).toHaveBeenCalledWith("race-2");
  });

  it("shows error state on API failure", async () => {
    server.use(
      http.get("/api/v1/races", () => {
        return new HttpResponse(null, { status: 500 });
      })
    );

    render(
      <RaceSelector selectedRaceId={null} onSelect={vi.fn()} />,
      { wrapper: createWrapper() }
    );

    expect(
      await screen.findByText("Failed to load races")
    ).toBeInTheDocument();
  });

  it("has correct aria-label for accessibility", async () => {
    render(
      <RaceSelector selectedRaceId={null} onSelect={vi.fn()} />,
      { wrapper: createWrapper() }
    );

    const select = await screen.findByLabelText("Select a race");
    expect(select).toBeInTheDocument();
  });
});
