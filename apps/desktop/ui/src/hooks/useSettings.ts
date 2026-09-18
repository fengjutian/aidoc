import { useEffect, useState } from "react";

export type FontSize = "sm" | "md" | "lg";
export type MermaidTheme = "auto" | "default" | "dark";

export interface Settings {
  autosave: boolean;
  fontSize: FontSize;
  mermaidTheme: MermaidTheme;
  openaiApiKey: string;
  openaiBaseUrl: string;
  openaiModel: string;
}

const STORAGE_KEY = "aidoc-settings";

const DEFAULT: Settings = {
  autosave: false,
  fontSize: "md",
  mermaidTheme: "auto",
  openaiApiKey: "",
  openaiBaseUrl: "",
  openaiModel: "",
};

function read(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT;
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      autosave: !!parsed.autosave,
      fontSize: ["sm", "md", "lg"].includes(parsed.fontSize as string)
        ? (parsed.fontSize as FontSize)
        : "md",
      mermaidTheme: ["auto", "default", "dark"].includes(
        parsed.mermaidTheme as string,
      )
        ? (parsed.mermaidTheme as MermaidTheme)
        : "auto",
      openaiApiKey: typeof parsed.openaiApiKey === "string" ? parsed.openaiApiKey : "",
      openaiBaseUrl:
        typeof parsed.openaiBaseUrl === "string" ? parsed.openaiBaseUrl : "",
      openaiModel: typeof parsed.openaiModel === "string" ? parsed.openaiModel : "",
    };
  } catch {
    return DEFAULT;
  }
}

export function useSettings() {
  const [settings, setSettings] = useState<Settings>(() => read());

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {
      // ignore
    }
  }, [settings]);

  return {
    settings,
    update: (patch: Partial<Settings>) =>
      setSettings((s) => ({ ...s, ...patch })),
  };
}