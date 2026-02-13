import { describe, it, expect } from "vitest";
import {
  formatTemp,
  formatWind,
  formatPrecip,
  formatDistance,
  formatDuration,
  windDirectionLabel,
  formatPercent,
  formatDate,
} from "./formatting";

describe("formatTemp", () => {
  it("formats positive temperature", () => {
    expect(formatTemp(5.3)).toBe("5°C");
  });

  it("formats negative temperature", () => {
    expect(formatTemp(-4.7)).toBe("-5°C");
  });

  it("formats zero", () => {
    expect(formatTemp(0)).toBe("0°C");
  });
});

describe("formatWind", () => {
  it("formats wind speed with one decimal", () => {
    expect(formatWind(3.2)).toBe("3.2 m/s");
  });

  it("formats zero wind", () => {
    expect(formatWind(0)).toBe("0.0 m/s");
  });
});

describe("formatPrecip", () => {
  it("formats precipitation with one decimal", () => {
    expect(formatPrecip(0.4)).toBe("0.4 mm");
  });
});

describe("formatDistance", () => {
  it("formats distance rounded to integer", () => {
    expect(formatDistance(11.3)).toBe("11 km");
  });
});

describe("formatDuration", () => {
  it("formats whole hours", () => {
    expect(formatDuration(8)).toBe("8h");
  });

  it("formats hours and minutes", () => {
    expect(formatDuration(7.5)).toBe("7h 30m");
  });
});

describe("windDirectionLabel", () => {
  it("returns N for 0 degrees", () => {
    expect(windDirectionLabel(0)).toBe("N");
  });

  it("returns NE for 45 degrees", () => {
    expect(windDirectionLabel(45)).toBe("NE");
  });

  it("returns S for 180 degrees", () => {
    expect(windDirectionLabel(180)).toBe("S");
  });

  it("returns NW for 315 degrees", () => {
    expect(windDirectionLabel(315)).toBe("NW");
  });

  it("returns N for 360 degrees", () => {
    expect(windDirectionLabel(360)).toBe("N");
  });
});

describe("formatPercent", () => {
  it("formats percentage rounded", () => {
    expect(formatPercent(82.3)).toBe("82%");
  });
});

describe("formatDate", () => {
  it("formats ISO date to readable date", () => {
    // 2026-03-01 is a Sunday
    expect(formatDate("2026-03-01T07:00:00Z")).toBe("Sun, 1 Mar 2026");
  });

  it("handles timezone correctly (Stockholm)", () => {
    // Late UTC time on Feb 28 is Mar 1 in Stockholm (UTC+1)
    expect(formatDate("2026-02-28T23:30:00Z")).toBe("Sun, 1 Mar 2026");
  });
});
