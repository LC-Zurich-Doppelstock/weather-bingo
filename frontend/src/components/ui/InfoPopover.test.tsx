import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import InfoPopover from "./InfoPopover";

describe("InfoPopover", () => {
  const content = "This is helpful info about the feature.";

  it("renders trigger button with (?) text", () => {
    render(<InfoPopover content={content} />);
    expect(screen.getByRole("button", { name: "More information" })).toBeInTheDocument();
    expect(screen.getByText("?")).toBeInTheDocument();
  });

  it("does not show popover content initially", () => {
    render(<InfoPopover content={content} />);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    expect(screen.queryByText(content)).not.toBeInTheDocument();
  });

  it("shows popover on click and hides on second click", () => {
    render(<InfoPopover content={content} />);
    const button = screen.getByRole("button", { name: "More information" });

    fireEvent.click(button);
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    expect(screen.getByText(content)).toBeInTheDocument();
    expect(button).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(button);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("closes popover when clicking outside", () => {
    render(
      <div>
        <InfoPopover content={content} />
        <span data-testid="outside">Outside</span>
      </div>
    );
    const button = screen.getByRole("button", { name: "More information" });

    fireEvent.click(button);
    expect(screen.getByRole("tooltip")).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("uses custom ariaLabel when provided", () => {
    render(<InfoPopover content={content} ariaLabel="Snow temperature info" />);
    expect(screen.getByRole("button", { name: "Snow temperature info" })).toBeInTheDocument();
  });

  it("has correct accessibility attributes", () => {
    render(<InfoPopover content={content} />);
    const button = screen.getByRole("button", { name: "More information" });
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).toHaveAttribute("aria-haspopup", "true");
  });
});
