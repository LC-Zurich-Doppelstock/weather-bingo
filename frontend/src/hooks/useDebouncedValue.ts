import { useEffect, useState } from "react";

/**
 * Returns a debounced version of `value` that only updates after
 * `delay` ms of inactivity. Useful for preventing excessive API
 * calls while the user is dragging a slider.
 */
export function useDebouncedValue<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debounced;
}
