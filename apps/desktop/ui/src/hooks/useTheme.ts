import { useEffect, useState } from "react";

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "aidoc-theme";

function resolve(theme: Theme): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return theme;
}

function readStored(): Theme {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === "light" || raw === "dark" || raw === "system") return raw;
  } catch {
    // localStorage unavailable (e.g. SSR); fall through.
  }
  return "system";
}

function apply(resolved: "light" | "dark") {
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  // Helps form controls / scrollbars pick up the palette.
  root.style.colorScheme = resolved;
}

/**
 * Theme controller. Persists user choice in localStorage and applies the
 * `.dark` class on `<html>`. When the choice is `system`, follows the OS
 * preference and re-resolves on media-query changes.
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => readStored());

  useEffect(() => {
    const resolved = resolve(theme);
    apply(resolved);

    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => apply(resolve("system"));
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme]);

  const setAndStore = (next: Theme) => {
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // ignore
    }
    setTheme(next);
  };

  return {
    theme,
    resolved: resolve(theme),
    set: setAndStore,
    cycle: () =>
      setAndStore(
        theme === "light" ? "dark" : theme === "dark" ? "system" : "light",
      ),
  };
}