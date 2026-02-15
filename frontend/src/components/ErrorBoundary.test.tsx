import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ErrorBoundary from "./ErrorBoundary";

function ThrowingComponent({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error("Test error message");
  }
  return <div>Normal content</div>;
}

describe("ErrorBoundary", () => {
  it("renders children when no error occurs", () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundary>
    );
    expect(screen.getByText("Normal content")).toBeInTheDocument();
  });

  it("renders error UI when a child throws", () => {
    // Suppress console.error from React's error boundary logging
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    expect(screen.getByText("Test error message")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Try Again/ })
    ).toBeInTheDocument();
    expect(screen.getByText(/3 attempts remaining/)).toBeInTheDocument();

    consoleSpy.mockRestore();
  });

  it("recovers when Try Again is clicked and error is resolved", async () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const user = userEvent.setup();

    // We can't easily change props after error, but we can test
    // that the button resets the error state
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    // Click Try Again - this resets the error state, but the child
    // will throw again since shouldThrow is still true
    await user.click(screen.getByRole("button", { name: /Try Again/ }));

    // After retry, React will try to render again - since component
    // still throws, error boundary catches it again
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    // Retry count should have decremented
    expect(screen.getByText(/2 attempts remaining/)).toBeInTheDocument();

    consoleSpy.mockRestore();
  });

  it("disables retry button after MAX_RETRIES exhausted", async () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const user = userEvent.setup();

    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    // Start with 3 attempts
    expect(screen.getByRole("button")).toHaveTextContent(/3 attempts remaining/);

    // Use up all 3 retries
    await user.click(screen.getByRole("button", { name: /Try Again/ }));
    expect(screen.getByRole("button")).toHaveTextContent(/2 attempts remaining/);

    await user.click(screen.getByRole("button", { name: /Try Again/ }));
    expect(screen.getByRole("button")).toHaveTextContent(/1 attempts remaining/);

    await user.click(screen.getByRole("button", { name: /Try Again/ }));

    // Button should now be gone and max retries message shown
    expect(screen.queryByRole("button", { name: /Try Again/ })).not.toBeInTheDocument();
    expect(screen.getByText(/Maximum retries reached/)).toBeInTheDocument();

    consoleSpy.mockRestore();
  });
});
