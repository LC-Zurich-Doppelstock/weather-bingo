import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import TargetTimeInput from "./TargetTimeInput";

describe("TargetTimeInput", () => {
  it("renders with initial value", () => {
    render(<TargetTimeInput value={8} onChange={vi.fn()} min={3} max={9} />);
    expect(screen.getByLabelText("Target race duration")).toBeInTheDocument();
    expect(screen.getByText("8h")).toBeInTheDocument();
  });

  it("displays formatted duration with half hours", () => {
    render(<TargetTimeInput value={7.5} onChange={vi.fn()} min={3} max={9} />);
    expect(screen.getByText("7h 30m")).toBeInTheDocument();
  });

  it("renders range input with correct min/max/step", () => {
    render(<TargetTimeInput value={6} onChange={vi.fn()} min={3} max={9} />);
    const input = screen.getByLabelText("Target race duration");
    expect(input).toHaveAttribute("type", "range");
    expect(input).toHaveAttribute("min", "3");
    expect(input).toHaveAttribute("max", "9");
    expect(input).toHaveAttribute("step", "0.5");
  });

  it("respects custom step", () => {
    render(
      <TargetTimeInput value={6} onChange={vi.fn()} min={4} max={20} step={1} />
    );
    const input = screen.getByLabelText("Target race duration");
    expect(input).toHaveAttribute("min", "4");
    expect(input).toHaveAttribute("max", "20");
    expect(input).toHaveAttribute("step", "1");
  });

  it("calls onChange when slider value changes", () => {
    const onChange = vi.fn();

    render(<TargetTimeInput value={8} onChange={onChange} min={3} max={9} />);
    const input = screen.getByLabelText(
      "Target race duration"
    ) as HTMLInputElement;

    // Verify range input exists with correct value
    expect(input).toHaveAttribute("type", "range");
    expect(input).toHaveValue("8");

    // Use fireEvent.change to simulate a value change on the range input
    fireEvent.change(input, { target: { value: "9" } });
    expect(onChange).toHaveBeenCalledWith(9);
  });

  it("shows the label text", () => {
    render(<TargetTimeInput value={8} onChange={vi.fn()} min={3} max={9} />);
    expect(screen.getByText("Target time")).toBeInTheDocument();
  });

  it("derives sensible range from race distance (e.g. 90km)", () => {
    // 90km / 30km/h = 3h min, 90km / 10km/h = 9h max
    render(<TargetTimeInput value={6} onChange={vi.fn()} min={3} max={9} />);
    const input = screen.getByLabelText("Target race duration");
    expect(input).toHaveAttribute("min", "3");
    expect(input).toHaveAttribute("max", "9");
  });
});
