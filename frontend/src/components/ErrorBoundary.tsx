import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  retryCount: number;
}

/** Maximum number of retry attempts before disabling the button. */
const MAX_RETRIES = 3;

/**
 * React Error Boundary that catches render errors and displays
 * a fallback UI instead of crashing the entire app.
 */
export default class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null, retryCount: 0 };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error("ErrorBoundary caught an error:", error, errorInfo);
  }

  handleRetry = (): void => {
    this.setState((prev) => ({
      hasError: false,
      error: null,
      retryCount: prev.retryCount + 1,
    }));
  };

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <div
          className="flex min-h-screen flex-col items-center justify-center bg-background p-8 text-center"
          role="alert"
        >
          <h1 className="mb-4 text-2xl font-bold text-error">
            Something went wrong
          </h1>
          <p className="mb-6 max-w-md text-text-secondary">
            An unexpected error occurred. Please try refreshing the page or
            click the button below.
          </p>
          {this.state.error && (
            <pre className="mb-6 max-w-lg overflow-auto rounded-lg bg-surface p-4 text-left text-xs text-text-muted">
              {this.state.error.message}
            </pre>
          )}
          {this.state.retryCount < MAX_RETRIES ? (
            <button
              onClick={this.handleRetry}
              className="rounded-lg bg-primary px-6 py-2 font-medium text-background transition-colors hover:bg-primary-hover"
            >
              Try Again ({MAX_RETRIES - this.state.retryCount} attempts remaining)
            </button>
          ) : (
            <p className="text-sm text-text-muted">
              Maximum retries reached. Please refresh the page.
            </p>
          )}
        </div>
      );
    }

    return this.props.children;
  }
}
