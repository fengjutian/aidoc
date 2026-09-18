import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  GitMerge,
  Link2,
  Link2Off,
  Loader2,
  Merge,
  Pencil,
  Plus,
  Split,
  Trash2,
  Undo2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

interface Change {
  node: string;
  change_type: string;
  summary: string | null;
  before_hash: string | null;
  after_hash: string | null;
}

interface RevisionDiffProps {
  revId: string | null;
  onClose: () => void;
}

const TYPE_META: Record<
  string,
  { label: string; icon: LucideIcon; tone: string }
> = {
  "content-update": {
    label: "Updated",
    icon: Pencil,
    tone: "bg-blue-500/10 text-blue-600 border-blue-500/30",
  },
  create: {
    label: "Created",
    icon: Plus,
    tone: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30",
  },
  delete: {
    label: "Deleted",
    icon: Trash2,
    tone: "bg-destructive/10 text-destructive border-destructive/30",
  },
  move: {
    label: "Moved",
    icon: Merge,
    tone: "bg-violet-500/10 text-violet-600 border-violet-500/30",
  },
  rename: {
    label: "Renamed",
    icon: Pencil,
    tone: "bg-amber-500/10 text-amber-600 border-amber-500/30",
  },
  split: {
    label: "Split",
    icon: Split,
    tone: "bg-cyan-500/10 text-cyan-600 border-cyan-500/30",
  },
  merge: {
    label: "Merged",
    icon: GitMerge,
    tone: "bg-indigo-500/10 text-indigo-600 border-indigo-500/30",
  },
  revert: {
    label: "Reverted",
    icon: Undo2,
    tone: "bg-orange-500/10 text-orange-600 border-orange-500/30",
  },
  "relation-add": {
    label: "Linked",
    icon: Link2,
    tone: "bg-teal-500/10 text-teal-600 border-teal-500/30",
  },
  "relation-remove": {
    label: "Unlinked",
    icon: Link2Off,
    tone: "bg-rose-500/10 text-rose-600 border-rose-500/30",
  },
};

const FALLBACK_META = {
  label: "Change",
  icon: Pencil,
  tone: "bg-muted text-muted-foreground border-border",
};

export function RevisionDiff({ revId, onClose }: RevisionDiffProps) {
  const [changes, setChanges] = useState<Change[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!revId) {
      setChanges(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setChanges(null);
    setError(null);
    (async () => {
      try {
        const list = await invoke<Change[]>("list_changes", { revId });
        if (!cancelled) setChanges(list);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [revId]);

  const counts = changes
    ? changes.reduce<Record<string, number>>((acc, c) => {
        acc[c.change_type] = (acc[c.change_type] ?? 0) + 1;
        return acc;
      }, {})
    : {};

  return (
    <Dialog open={!!revId} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="flex items-center gap-2">
            <span>Revision</span>
            <code className="rounded bg-muted px-2 py-0.5 font-mono text-sm">
              {revId}
            </code>
          </DialogTitle>
          <DialogDescription>
            Per-node changes recorded by this revision. History is immutable.
          </DialogDescription>
        </DialogHeader>

        <div className="border-b bg-muted/30 px-5 py-2.5">
          {changes == null && error == null && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Loading changes…
            </div>
          )}
          {error && (
            <div className="text-sm text-destructive">Failed: {error}</div>
          )}
          {changes && (
            <div className="flex flex-wrap items-center gap-2 text-xs">
              {Object.entries(counts).map(([k, n]) => {
                const meta = TYPE_META[k] ?? FALLBACK_META;
                const Icon = meta.icon;
                return (
                  <span
                    key={k}
                    className={cn(
                      "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-medium",
                      meta.tone,
                    )}
                  >
                    <Icon className="h-3 w-3" />
                    {meta.label}
                    <span className="ml-1 opacity-60">{n}</span>
                  </span>
                );
              })}
              <span className="ml-auto text-muted-foreground">
                {changes.length} change{changes.length === 1 ? "" : "s"}
              </span>
            </div>
          )}
        </div>

        <ScrollArea className="max-h-[60vh]">
          {changes && changes.length === 0 && (
            <div className="px-5 py-8 text-center text-sm text-muted-foreground">
              No node-level changes recorded for this revision.
            </div>
          )}
          {changes && changes.length > 0 && (
            <ul className="divide-y">
              {changes.map((c, i) => {
                const meta = TYPE_META[c.change_type] ?? FALLBACK_META;
                const Icon = meta.icon;
                return (
                  <li key={`${c.node}-${i}`} className="flex items-start gap-3 px-5 py-3">
                    <span
                      className={cn(
                        "mt-0.5 inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-md border",
                        meta.tone,
                      )}
                    >
                      <Icon className="h-3.5 w-3.5" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <code className="truncate font-mono text-sm">
                          {c.node}
                        </code>
                        <span className="text-xs uppercase tracking-wider text-muted-foreground">
                          {meta.label}
                        </span>
                      </div>
                      {c.summary && (
                        <div className="mt-0.5 text-sm text-muted-foreground">
                          {c.summary}
                        </div>
                      )}
                      {(c.before_hash || c.after_hash) && (
                        <div className="mt-1 flex gap-3 font-mono text-[10px] text-muted-foreground">
                          {c.before_hash && (
                            <span>
                              before: <code>{shortHash(c.before_hash)}</code>
                            </span>
                          )}
                          {c.after_hash && (
                            <span>
                              after: <code>{shortHash(c.after_hash)}</code>
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </ScrollArea>

        <div className="flex items-center justify-end gap-2 border-t bg-muted/30 px-5 py-3">
          <Button variant="outline" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function shortHash(h: string): string {
  return h.length > 12 ? `${h.slice(0, 8)}…${h.slice(-4)}` : h;
}