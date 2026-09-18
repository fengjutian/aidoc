import { AlertCircle, CheckCircle2, CircleDashed, Loader2 } from "lucide-react";

export type SaveStatusState = "idle" | "saving" | "saved" | "dirty" | "never";

interface SaveStatusProps {
  state: SaveStatusState;
  lastSavedAt: Date | null;
}

function formatRelative(d: Date, now: Date): string {
  const diff = Math.max(0, Math.round((now.getTime() - d.getTime()) / 1000));
  if (diff < 5) return "just now";
  if (diff < 60) return `${diff}s ago`;
  const m = Math.round(diff / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  return d.toLocaleString();
}

export function SaveStatus({ state, lastSavedAt }: SaveStatusProps) {
  if (state === "saving") {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-full bg-muted/60 px-2.5 py-1 text-xs text-muted-foreground">
        <Loader2 className="h-3 w-3 animate-spin" />
        Saving…
      </span>
    );
  }

  if (state === "dirty") {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-500/10 px-2.5 py-1 text-xs text-amber-600 dark:text-amber-400">
        <AlertCircle className="h-3 w-3" />
        Unsaved changes
      </span>
    );
  }

  if (state === "saved" && lastSavedAt) {
    return (
      <span
        className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-2.5 py-1 text-xs text-emerald-600 dark:text-emerald-400"
        title={lastSavedAt.toLocaleString()}
      >
        <CheckCircle2 className="h-3 w-3" />
        Saved {formatRelative(lastSavedAt, new Date())}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-muted/60 px-2.5 py-1 text-xs text-muted-foreground">
      <CircleDashed className="h-3 w-3" />
      Unsaved
    </span>
  );
}