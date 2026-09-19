import { useEffect, useState } from "react";

export type FontSize = "sm" | "md" | "lg";
export type MermaidTheme = "auto" | "default" | "dark";

export type AIProvider = "openai" | "ollama" | "custom";

export interface Settings {
  autosave: boolean;
  fontSize: FontSize;
  mermaidTheme: MermaidTheme;
  aiProvider: AIProvider;
  openaiApiKey: string;
  openaiBaseUrl: string;
  openaiModel: string;
}

const STORAGE_KEY = "aidoc-settings";

/** Sensible base URL defaults per provider; empty `openaiBaseUrl` lets the
 *  provider pick. */
const PROVIDER_BASE_URLS: Record<AIProvider, string> = {
  openai: "https://api.openai.com/v1",
  ollama: "http://localhost:11434/v1",
  custom: "",
};

const PROVIDER_DEFAULT_MODELS: Record<AIProvider, string> = {
  openai: "gpt-4o-mini",
  ollama: "llama3.2",
  custom: "",
};

const DEFAULT: Settings = {
  autosave: false,
  fontSize: "md",
  mermaidTheme: "auto",
  aiProvider: "openai",
  openaiApiKey: "",
  openaiBaseUrl: PROVIDER_BASE_URLS.openai,
  openaiModel: PROVIDER_DEFAULT_MODELS.openai,
};

function read(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT;
    const parsed = JSON.parse(raw) as Partial<Settings>;
    const provider: AIProvider =
      parsed.aiProvider === "ollama" || parsed.aiProvider === "custom"
        ? parsed.aiProvider
        : "openai";
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
      aiProvider: provider,
      openaiApiKey: typeof parsed.openaiApiKey === "string" ? parsed.openaiApiKey : "",
      openaiBaseUrl:
        typeof parsed.openaiBaseUrl === "string"
          ? parsed.openaiBaseUrl
          : PROVIDER_BASE_URLS[provider],
      openaiModel: typeof parsed.openaiModel === "string"
        ? parsed.openaiModel
        : PROVIDER_DEFAULT_MODELS[provider],
    };
  } catch {
    return DEFAULT;
  }
}

/**
 * Resolve the effective `base_url` / `api_key` / `model` for the current
 * provider. Returns the user override if set, otherwise the provider's
 * defaults. Callers should prefer these values over `settings.openai*`
 * so that switching providers stays consistent.
 */
export function resolveProviderDefaults(s: Settings): {
  baseUrl: string;
  model: string;
} {
  return {
    baseUrl: s.openaiBaseUrl || PROVIDER_BASE_URLS[s.aiProvider],
    model: s.openaiModel || PROVIDER_DEFAULT_MODELS[s.aiProvider],
  };
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