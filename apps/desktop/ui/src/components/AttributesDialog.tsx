import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Plus, Save, Tag, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";

interface AttributesDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  nodeId: string | null;
  initial: Record<string, string>;
  onSaved: () => void;
}

interface Row {
  key: string;
  value: string;
}

function rowsFromRecord(rec: Record<string, string>): Row[] {
  return Object.entries(rec)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => ({ key, value }));
}

function recordFromRows(rows: Row[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of rows) {
    const k = r.key.trim();
    if (!k) continue;
    out[k] = r.value;
  }
  return out;
}

export function AttributesDialog({
  open,
  onOpenChange,
  nodeId,
  initial,
  onSaved,
}: AttributesDialogProps) {
  const [rows, setRows] = useState<Row[]>(() => rowsFromRecord(initial));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setRows(rowsFromRecord(initial));
      setError(null);
    }
  }, [open, initial, nodeId]);

  const addRow = () => setRows((rs) => [...rs, { key: "", value: "" }]);
  const removeRow = (i: number) => setRows((rs) => rs.filter((_, idx) => idx !== i));
  const updateRow = (i: number, patch: Partial<Row>) =>
    setRows((rs) => rs.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));

  const save = async () => {
    if (!nodeId) return;
    const attrs = recordFromRows(rows);
    setBusy(true);
    setError(null);
    try {
      await invoke<string>("set_node_attributes", { target: nodeId, attrs });
      onSaved();
      onOpenChange(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const dirty =
    JSON.stringify(recordFromRows(rows)) !== JSON.stringify(initial ?? {});
  const hasBlankKey = rows.some((r) => !r.key.trim());

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="flex items-center gap-2">
            <Tag className="h-4 w-4 text-primary" />
            Attributes for{" "}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-sm">
              {nodeId ?? "—"}
            </code>
          </DialogTitle>
          <DialogDescription>
            Free-form key/value metadata attached to this node. Empty keys are
            ignored on save.
          </DialogDescription>
        </DialogHeader>

        <ScrollArea className="max-h-[50vh]">
          <div className="px-5 py-3">
            {rows.length === 0 ? (
              <div className="rounded-md border border-dashed bg-background/50 px-3 py-6 text-sm text-muted-foreground">
                No attributes yet.
              </div>
            ) : (
              <ul className="space-y-2">
                {rows.map((r, i) => (
                  <li key={i} className="flex items-center gap-2">
                    <input
                      className="h-9 w-40 rounded-md border border-input bg-background px-3 text-sm font-mono shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
                      placeholder="key"
                      value={r.key}
                      onChange={(e) => updateRow(i, { key: e.target.value })}
                      disabled={busy}
                    />
                    <span className="text-xs text-muted-foreground">=</span>
                    <input
                      className="h-9 flex-1 rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
                      placeholder="value"
                      value={r.value}
                      onChange={(e) => updateRow(i, { value: e.target.value })}
                      disabled={busy}
                    />
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-8 w-8 p-0 text-muted-foreground hover:text-destructive"
                      onClick={() => removeRow(i)}
                      disabled={busy}
                      aria-label={`Remove ${r.key || "row"}`}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
            {error && (
              <div className="mt-3 text-xs text-destructive">{error}</div>
            )}
          </div>
        </ScrollArea>

        <div className="flex items-center justify-between border-t bg-muted/30 px-5 py-3">
          <Button size="sm" variant="outline" onClick={addRow} disabled={busy}>
            <Plus className="mr-1.5 h-3.5 w-3.5" />
            Add row
          </Button>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button
              size="sm"
              onClick={() => void save()}
              disabled={!dirty || busy || hasBlankKey}
            >
              {busy ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Save className="mr-1.5 h-3.5 w-3.5" />
              )}
              Save
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}