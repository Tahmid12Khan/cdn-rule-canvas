"use client";

// Client hook: debounces a changing value (browser timers + React state).
import { useEffect, useState } from "react";

/**
 * Returns `value` after it has been stable for `delayMs`. Used by the version
 * search input to throttle the server-side `?search=` query (300ms per
 * FRONTEND CONTRACT §2.3).
 */
export function useDebouncedValue<T>(value: T, delayMs = 300): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(id);
  }, [value, delayMs]);

  return debounced;
}
