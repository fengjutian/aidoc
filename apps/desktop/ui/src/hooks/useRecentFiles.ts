import { useCallback, useEffect, useState } from "react";

export const STORAGE_KEY = "aidoc-recent-files";
export const MAX_ENTRIES = 8;

/** Pure helper extracted from the hook so it can be unit-tested without React. */
export function mergeRecent(
  entries: string[],
  path: string,
  max: number = MAX_ENTRIES,
): string[] {
  const filtered = entries.filter((p) => p !== path);
  return [path, ...filtered].slice(0, max);
}

/** Pure helper: parse a localStorage blob, fall back to []. */
export function parseStored(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((v): v is string => typeof v === "string")
      : [];
  } catch {
    return [];
  }
}

function read(): string[] {
  return parseStored(localStorage.getItem(STORAGE_KEY));
}

function write(entries: string[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // ignore quota / privacy errors
  }
}

/**
 * Recent `.aidoc` paths the user has opened/initialized. Capped at
 * `MAX_ENTRIES`; the most-recent path is always at index 0.
 */
export function useRecentFiles() {
  const [recent, setRecent] = useState<string[]>(() => read());

  useEffect(() => {
    write(recent);
  }, [recent]);

  const addOpened = useCallback((path: string) => {
    setRecent((prev) => {
      const filtered = prev.filter((p) => p !== path);
      return [path, ...filtered].slice(0, MAX_ENTRIES);
    });
  }, []);

  const remove = useCallback((path: string) => {
    setRecent((prev) => prev.filter((p) => p !== path));
  }, []);

  const clear = useCallback(() => setRecent([]), []);

  return { recent, addOpened, remove, clear };
}