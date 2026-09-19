import en from "../i18n/en.json";
import zhCN from "../i18n/zh-CN.json";

/**
 * Supported message catalogs. Adding a language = dropping a JSON file
 * here and unioning the literal into `Locale`.
 */
const CATALOGS = {
  en,
  "zh-CN": zhCN,
} as const;

export type Locale = keyof typeof CATALOGS;

const LANG_KEY = "aidoc-language";

/**
 * Pick the closest catalog for the running browser / OS. Falls back to
 * English when nothing matches.
 */
export function detectLocale(): Locale {
  const candidates =
    typeof navigator !== "undefined"
      ? [navigator.language, ...(navigator.languages ?? []), "en"]
      : ["en"];
  for (const raw of candidates) {
    const lc = raw.toLowerCase();
    if (lc.startsWith("zh")) return "zh-CN";
    if (lc.startsWith("en")) return "en";
  }
  return "en";
}

export function readSavedLocale(): Locale | "auto" {
  if (typeof localStorage === "undefined") return "auto";
  const raw = localStorage.getItem(LANG_KEY);
  if (raw === "en" || raw === "zh-CN") return raw;
  return "auto";
}

export function writeSavedLocale(loc: Locale | "auto") {
  if (typeof localStorage === "undefined") return;
  if (loc === "auto") localStorage.removeItem(LANG_KEY);
  else localStorage.setItem(LANG_KEY, loc);
}

/**
 * Look up a dotted key in a catalog and substitute `{name}` placeholders
 * from `vars`. Returns the dotted key itself if missing so callers see
 * "ui.foo.bar" in dev rather than a blank string.
 */
function format(
  catalog: Record<string, string>,
  key: string,
  vars?: Record<string, string | number>,
): string {
  const raw = catalog[key];
  if (raw === undefined) return key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (_, name) =>
    vars[name] !== undefined ? String(vars[name]) : `{${name}}`,
  );
}

let cachedDefault: Locale | null = null;
export function createT(locale: Locale | "auto"): (key: string, vars?: Record<string, string | number>) => string {
  const resolved = locale === "auto" ? (cachedDefault ?? detectLocale()) : locale;
  if (locale === "auto") cachedDefault = resolved;
  const catalog = (CATALOGS[resolved] ?? CATALOGS.en) as Record<string, string>;
  return (key, vars) => format(catalog, key, vars);
}