import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, Check, Loader2, Wand2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface DiffPreviewDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Pre-filled op JSON (e.g. the AI just proposed an op). */
  initialOpJson?: string;
  /** Called after the user accepts and the op is applied. The parent should
   *  refresh the document and dismiss the dialog. */
  onApplied?: (revision: string) => void;
}

interface NodeSummary {
  id: string;
  kind: string;
  parent: string | null;
  position: number;
  content: string;
}

interface NodeDiff {
  status: "added" | "removed" | "modified";
  node?: NodeSummary;
  before?: NodeSummary;
  after?: NodeSummary;
}

interface RelationDiff {
  status: "added" | "removed";
  relation_id: string;
  source: string;
  target: string;
  kind: string;
}

interface PreviewReport {
  current_revision: string;
  next_revision: string;
  summary: string;
  nodes: [string, NodeDiff][];
  relations: [string, RelationDiff][];
}

/**
 * Modal that previews an AI-proposed operation against the live document.
 *
 * Flow:
 *   1. User opens the dialog (typically from the AI Chat panel). An op JSON
 *      is pre-filled — usually the one the AI just streamed.
 *   2. Click "Preview" → `preview_operation_json` returns a diff.
 *   3. The diff lists every node that would be added / removed / modified and
 *      every relation that would change.
 *   4. "Apply" sends the same op JSON to `apply_operation_json`. The live
 *      document is updated only after the user explicitly accepts.
 *   5. "Cancel" dismisses the dialog without any changes.
 */
export function DiffPreviewDialog({
  open,
  onOpenChange,
  initialOpJson = "",
  onApplied,
}: DiffPreviewDialogProps) {
  const [opText, setOpText] = useState(initialOpJson);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<PreviewReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Reset state whenever the dialog is reopened.
  useEffect(() => {
    if (open) {
      setOpText(initialOpJson);
      setReport(null);
      setError(null);
    }
  }, [open, initialOpJson]);

  const runPreview = async () => {
    setBusy(true);
    setError(null);
    try {
      const parsed = JSON.parse(opText) as unknown;
      const result = await invoke<PreviewReport>("preview_operation_json", {
        opJson: parsed,
      });
      setReport(result);
    } catch (e) {
      setError(String(e));
      setReport(null);
    } finally {
      setBusy(false);
    }
  };

  const runApply = async () => {
    setBusy(true);
    setError(null);
    try {
      const parsed = JSON.parse(opText) as unknown;
      const rev = await invoke<string>("apply_operation_json", { opJson: parsed });
      onApplied?.(rev);
      onOpenChange(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogTitle className="flex items-center gap-2">
          <Wand2 className="h-4 w-4 text-primary" />
          Preview AI-proposed operation
        </DialogTitle>
        <DialogDescription>
          The store is not modified until you click <strong>Apply</strong>.
          The diff below shows what the operation would change if applied.
        </DialogDescription>

        <div className="grid gap-3">
          <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Operation JSON
          </label>
          <textarea
            className="h-40 w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-xs shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
            value={opText}
            onChange={(e) => setOpText(e.target.value)}
            spellCheck={false}
          />
          <div className="flex gap-2">
            <Button onClick={runPreview} disabled={busy || !opText.trim()} variant="secondary">
              {busy && !report ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Wand2 className="mr-2 h-4 w-4" />
              )}
              Preview
            </Button>
          </div>
        </div>

        {error && (
          <div className="flex items-center gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            <AlertCircle className="h-4 w-4" />
            <span className="font-mono text-xs">{error}</span>
          </div>
        )}

        {report && (
          <div className="grid gap-3">
            <div className="rounded-md border bg-muted/30 px-3 py-2 text-sm">
              <code className="font-mono text-xs">{report.summary}</code>
              <div className="mt-1 text-xs text-muted-foreground">
                current = <code>{report.current_revision}</code> → next ={" "}
                <code>{report.next_revision}</code>
              </div>
            </div>

            <div className="grid gap-2">
              <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Nodes ({report.nodes.length})
              </h4>
              {report.nodes.length === 0 && (
                <div className="rounded-md border border-dashed px-3 py-2 text-xs text-muted-foreground">
                  No node changes.
                </div>
              )}
              {report.nodes.map(([id, diff]) => (
                <NodeDiffRow key={id} id={id} diff={diff} />
              ))}
            </div>

            <div className="grid gap-2">
              <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Relations ({report.relations.length})
              </h4>
              {report.relations.length === 0 && (
                <div className="rounded-md border border-dashed px-3 py-2 text-xs text-muted-foreground">
                  No relation changes.
                </div>
              )}
              {report.relations.map(([id, diff]) => (
                <div
                  key={id}
                  className="flex items-center justify-between rounded-md border bg-background px-3 py-2 text-xs"
                >
                  <span className="font-mono">{id}</span>
                  <span className="flex items-center gap-2 text-muted-foreground">
                    <span>
                      {diff.source} → {diff.target} ({diff.kind})
                    </span>
                    <span
                      className={cn(
                        "rounded px-1.5 py-0.5 text-[10px] font-medium uppercase",
                        diff.status === "added" && "bg-emerald-500/15 text-emerald-600",
                        diff.status === "removed" && "bg-destructive/15 text-destructive",
                      )}
                    >
                      {diff.status}
                    </span>
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        <DialogFooter className="gap-2">
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={busy}>
            <X className="mr-2 h-4 w-4" />
            Cancel
          </Button>
          <Button onClick={runApply} disabled={busy || !report}>
            {busy && report ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Check className="mr-2 h-4 w-4" />
            )}
            Apply
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function NodeDiffRow({ id, diff }: { id: string; diff: NodeDiff }) {
  const badge =
    diff.status === "added"
      ? "bg-emerald-500/15 text-emerald-600"
      : diff.status === "removed"
        ? "bg-destructive/15 text-destructive"
        : "bg-amber-500/15 text-amber-700";

  const headline = (() => {
    const node = diff.after ?? diff.before ?? diff.node;
    if (!node) return id;
    return `${id} · ${node.kind}${node.parent ? ` (child of ${node.parent})` : ""}`;
  })();

  return (
    <div className="rounded-md border bg-background px-3 py-2 text-xs">
      <div className="flex items-center justify-between">
        <span className="font-mono">{headline}</span>
        <span
          className={cn(
            "rounded px-1.5 py-0.5 text-[10px] font-medium uppercase",
            badge,
          )}
        >
          {diff.status}
        </span>
      </div>
      {diff.status === "modified" && diff.before && diff.after && (
        <div className="mt-2 grid grid-cols-2 gap-2">
          <pre className="overflow-auto rounded bg-destructive/5 px-2 py-1 font-mono text-[11px] whitespace-pre-wrap break-words">
            {diff.before.content}
          </pre>
          <pre className="overflow-auto rounded bg-emerald-500/5 px-2 py-1 font-mono text-[11px] whitespace-pre-wrap break-words">
            {diff.after.content}
          </pre>
        </div>
      )}
      {(diff.status === "added" || diff.status === "removed") && diff.node && (
        <pre className="mt-2 overflow-auto rounded bg-muted/40 px-2 py-1 font-mono text-[11px] whitespace-pre-wrap break-words">
          {diff.node.content}
        </pre>
      )}
    </div>
  );
}