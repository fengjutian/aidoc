import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Link2, Link2Off, Loader2, Plus } from "lucide-react";

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

interface NodeRow {
  id: string;
  kind: string;
  parent: string | null;
  position: number;
  content: string;
}

interface RelationRow {
  id: string;
  source: string;
  target: string;
  kind: string;
}

interface LinkDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Source node id — locked for the "create" half of the dialog. */
  source: string | null;
  /** All nodes, used for target picker + relation list. */
  nodes: NodeRow[];
  /** Existing relations, used to render current links for the source node. */
  relations: RelationRow[];
  /** Called after any mutation so the parent can refresh. */
  onChanged: () => void;
}

export function LinkDialog({
  open,
  onOpenChange,
  source,
  nodes,
  relations,
  onChanged,
}: LinkDialogProps) {
  const [target, setTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setTarget("");
      setError(null);
    }
  }, [open, source]);

  const candidates = nodes
    .filter((n) => n.id !== source)
    .sort((a, b) => a.position - b.position);

  const outgoing = source
    ? relations.filter((r) => r.source === source)
    : [];
  const incoming = source
    ? relations.filter((r) => r.target === source)
    : [];

  const createLink = async () => {
    if (!source) {
      setError("No source node selected.");
      return;
    }
    const t = target.trim();
    if (!t) {
      setError("Target node id is required.");
      return;
    }
    if (t === source) {
      setError("Source and target must differ.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await invoke<string>("create_link", { source, target: t });
      setTarget("");
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const deleteLink = async (relTarget: string) => {
    if (!source) return;
    setBusy(true);
    setError(null);
    try {
      await invoke<string>("delete_link", { source, target: relTarget });
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="flex items-center gap-2">
            <Link2 className="h-4 w-4 text-primary" />
            Links from <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-sm">{source ?? "—"}</code>
          </DialogTitle>
          <DialogDescription>
            Create a directional link to another node. Existing links are listed below.
          </DialogDescription>
        </DialogHeader>

        <div className="border-b bg-muted/30 px-5 py-3">
          <div className="flex gap-2">
            <select
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              disabled={!source || busy}
            >
              <option value="">Choose a target node…</option>
              {candidates.map((n) => (
                <option key={n.id} value={n.id}>
                  {n.id}  ({n.kind})
                </option>
              ))}
            </select>
            <Button
              size="sm"
              onClick={() => void createLink()}
              disabled={!source || !target || busy}
            >
              {busy ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Plus className="mr-1.5 h-3.5 w-3.5" />
              )}
              Link
            </Button>
          </div>
          {error && (
            <div className="mt-2 text-xs text-destructive">{error}</div>
          )}
        </div>

        <ScrollArea className="max-h-[40vh]">
          <div className="px-5 py-3">
            <Section
              title="Outgoing"
              empty="No outgoing links."
              items={outgoing}
              renderItem={(r) => (
                <button
                  type="button"
                  key={r.id}
                  onClick={() => setTarget(r.target)}
                  disabled={busy}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm hover:bg-accent"
                >
                  <ArrowRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <code className="truncate font-mono">{r.target}</code>
                  <span className="ml-auto text-[10px] uppercase text-muted-foreground">
                    {r.kind}
                  </span>
                </button>
              )}
              renderActions={(r) => (
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
                  onClick={() => void deleteLink(r.target)}
                  disabled={busy}
                  aria-label={`Unlink ${r.target}`}
                >
                  <Link2Off className="h-3.5 w-3.5" />
                </Button>
              )}
            />

            <div className="my-3 h-px bg-border" />

            <Section
              title="Incoming"
              empty="No incoming links."
              items={incoming}
              renderItem={(r) => (
                <div
                  key={r.id}
                  className="flex items-center gap-2 rounded-md px-2 py-1 text-sm"
                >
                  <code className="truncate font-mono">{r.source}</code>
                  <ArrowRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <span className="text-xs text-muted-foreground">this</span>
                  <span className="ml-auto text-[10px] uppercase text-muted-foreground">
                    {r.kind}
                  </span>
                </div>
              )}
            />
          </div>
        </ScrollArea>

        <div className="flex items-center justify-end border-t bg-muted/30 px-5 py-3">
          <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

interface SectionProps {
  title: string;
  empty: string;
  items: RelationRow[];
  renderItem: (r: RelationRow) => React.ReactNode;
  renderActions?: (r: RelationRow) => React.ReactNode;
}

function Section({
  title,
  empty,
  items,
  renderItem,
  renderActions,
}: SectionProps) {
  return (
    <div>
      <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        {title} ({items.length})
      </div>
      {items.length === 0 ? (
        <div className={cn("rounded-md border border-dashed bg-background/50 px-3 py-3 text-center text-xs text-muted-foreground")}>
          {empty}
        </div>
      ) : (
        <ul className="space-y-0.5">
          {items.map((r) => (
            <li key={r.id} className="flex items-center gap-1">
              <div className="min-w-0 flex-1">{renderItem(r)}</div>
              {renderActions?.(r)}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
