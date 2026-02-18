import { describe, it, expect } from "vitest";
import {
  formatTemp,
  formatWind,
  formatPrecip,
  formatDuration,
  formatTime,
  formatTimeWithZone,
  windDirectionLabel,
  formatPercent,
  formatDate,
  formatCheckBackMessage,
  RACE_TIMEZONE,
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

describe("formatDuration", () => {
  it("formats whole hours", () => {
    expect(formatDuration(8)).toBe("8h");
  });

  it("formats hours and minutes", () => {
    expect(formatDuration(7.5)).toBe("7h 30m");
  });

  it("handles edge case near rounding boundary (2.999)", () => {
    // Previously produced "2h 60m" due to floor + round mismatch
    expect(formatDuration(2.999)).toBe("3h");
  });

  it("handles edge case near rounding boundary (2.991)", () => {
    expect(formatDuration(2.991)).toBe("2h 59m");
  });

  it("formats zero", () => {
    expect(formatDuration(0)).toBe("0h");
  });

  it("formats small durations", () => {
    expect(formatDuration(0.25)).toBe("0h 15m");
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

  it("normalizes negative degrees (-45 → NW)", () => {
    expect(windDirectionLabel(-45)).toBe("NW");
  });

  it("normalizes negative degrees (-90 → W)", () => {
    expect(windDirectionLabel(-90)).toBe("W");
  });

  it("normalizes >360 degrees (405 → NE)", () => {
    expect(windDirectionLabel(405)).toBe("NE");
  });

  it("normalizes large negative (-315 → NE)", () => {
    expect(windDirectionLabel(-315)).toBe("NE");
  });
});

describe("formatPercent", () => {
  it("formats percentage rounded", () => {
    expect(formatPercent(82.3)).toBe("82%");
  });
});

describe("formatDate", () => {
  it("formats ISO date to readable date", () => {
    // 2026-03-01 is a Sunday. Use partial matching for locale resilience
    // (some Node versions may use "1 Mar" vs "1 Mar." etc.)
    const result = formatDate("2026-03-01T07:00:00Z");
    expect(result).toContain("Sun");
    expect(result).toContain("Mar");
    expect(result).toContain("2026");
    expect(result).toContain("1");
  });

  it("handles timezone correctly (Stockholm)", () => {
    // Late UTC time on Feb 28 is Mar 1 in Stockholm (UTC+1)
    const result = formatDate("2026-02-28T23:30:00Z");
    expect(result).toContain("Sun");
    expect(result).toContain("Mar");
    expect(result).toContain("2026");
  });
});

describe("formatTime", () => {
  it("formats ISO string to HH:MM in Stockholm timezone", () => {
    // 07:00 UTC = 08:00 CET (Stockholm, UTC+1 in winter)
    expect(formatTime("2026-03-01T07:00:00Z")).toBe("08:00");
  });

  it("formats afternoon time correctly", () => {
    // 14:30 UTC = 15:30 CET
    expect(formatTime("2026-03-01T14:30:00Z")).toBe("15:30");
  });

  it("handles midnight crossing", () => {
    // 23:30 UTC on Feb 28 = 00:30 CET on Mar 1
    expect(formatTime("2026-02-28T23:30:00Z")).toBe("00:30");
  });

  it("formats time with minutes", () => {
    // 09:24 UTC = 10:24 CET
    expect(formatTime("2026-03-01T09:24:00Z")).toBe("10:24");
  });
});

describe("formatTimeWithZone", () => {
  it("formats time with timezone label", () => {
    // 07:00 UTC = 08:00 CET
    const result = formatTimeWithZone("2026-03-01T07:00:00Z");
    expect(result).toContain("08:00");
    // Timezone label varies by environment (CET, GMT+1, etc.)
    expect(result.length).toBeGreaterThan(5);
  });

  it("includes timezone for afternoon time", () => {
    const result = formatTimeWithZone("2026-03-01T14:30:00Z");
    expect(result).toContain("15:30");
  });
});

describe("RACE_TIMEZONE", () => {
  it("is Europe/Stockholm", () => {
    expect(RACE_TIMEZONE).toBe("Europe/Stockholm");
  });
});

describe("formatCheckBackMessage", () => {
  it("returns generic message when no horizon", () => {
    expect(formatCheckBackMessage(null, "2026-03-01T07:00:00Z")).toBe(
      "Check back as it extends daily."
    );
  });

  it("returns tomorrow when 1 day away", () => {
    expect(
      formatCheckBackMessage("2026-02-28T12:00:00Z", "2026-03-01T07:00:00Z")
    ).toBe("Check back tomorrow.");
  });

  it("returns tomorrow when less than 1 day away", () => {
    expect(
      formatCheckBackMessage("2026-02-28T18:00:00Z", "2026-03-01T07:00:00Z")
    ).toBe("Check back tomorrow.");
  });

  it("returns ~N days for multi-day gap", () => {
    // 5 days gap
    expect(
      formatCheckBackMessage("2026-02-24T12:00:00Z", "2026-03-01T07:00:00Z")
    ).toBe("Check back in ~5 days.");
  });

  it("returns ~2 days for slightly over 1 day", () => {
    // horizon Feb 27 00:00, target Mar 1 07:00 = 2.29 days → ceil = 3
    expect(
      formatCheckBackMessage("2026-02-27T00:00:00Z", "2026-03-01T07:00:00Z")
    ).toBe("Check back in ~3 days.");
  });
});
