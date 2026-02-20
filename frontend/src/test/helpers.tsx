/**
 * Shared test helpers: QueryClient wrapper and MSW lifecycle setup.
 *
 * Eliminates the identical `createWrapper()` and `beforeAll/afterEach/afterAll`
 * blocks duplicated across multiple test files.
 */

import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { SetupServer } from "msw/node";
import { afterAll, afterEach, beforeAll } from "vitest";

/**
 * Creates a fresh QueryClientProvider wrapper with retry disabled.
 * Use as the `wrapper` option in RTL `render()`.
 */
export function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

/**
 * Registers MSW lifecycle hooks (listen / resetHandlers / close).
 * Call once at the top level of each test file that uses MSW.
 *
 * ```ts
 * const server = setupServer(...);
 * setupMswLifecycle(server);
 * ```
 */
export function setupMswLifecycle(server: SetupServer): void {
  beforeAll(() => server.listen());
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());
}
