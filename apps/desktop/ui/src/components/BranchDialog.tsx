import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GitBranch, GitMerge, Loader2, Plus } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface BranchRow {
  name: string;
  head: string | null;
  revisions: number;
  current: boolean;
}

interface BranchDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Called after any mutation so the parent can refresh revisions. */
  onChanged: () => void;
}

export function BranchDialog({ open, onOpenChange, onChanged }: BranchDialogProps) {
  const [branches, setBranches] = useState<BranchRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = async () => {
    if (!open) return;
    try {
      const list = await invoke<BranchRow[]>("list_branches");
      setBranches(list);
    } catch (e) {
      setError(String(e));
      setBranches([]);
    }
  };

  useEffect(() => {
    if (open) {
      setError(null);
      setNewName("");
      void refresh();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const createBranch = async () => {
    const name = newName.trim();
    if (!name) {
      setError("Branch name is required.");
      return;
    }
    setBusy(`create:${name}`);
    setError(null);
    try {
      await invoke<string>("create_branch", { name });
      setNewName("");
      await refresh();
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const mergeBranch = async (name: string) => {
    if (name === "main") return;
    setBusy(`merge:${name}`);
    setError(null);
    try {
      await invoke<string>("merge_branch", { name });
      await refresh();
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const checkoutBranch = async (name: string) => {
    setBusy(`checkout:${name}`);
    setError(null);
    try {
      await invoke<string>("checkout_branch", { name });
      await refresh();
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const sortedBranches: BranchRow[] = branches
    ? [...branches].sort((a, b) => {
        if (a.name === "main") return -1;
        if (b.name === "main") return 1;
        return a.name.localeCompare(b.name);
      })
    : [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="flex items-center gap-2">
            <GitBranch className="h-4 w-4 text-primary" />
            Branches
          </DialogTitle>
          <DialogDescription>
            Create a branch, switch between branches, or merge into main after checking out main.
          </DialogDescription>
        </DialogHeader>

        <div className="border-b bg-muted/30 px-5 py-3">
          <div className="flex gap-2">
            <input
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
              placeholder="new-branch-name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void createBranch();
                }
              }}
            />
            <Button
              size="sm"
              onClick={() => void createBranch()}
              disabled={!newName.trim() || busy !== null}
            >
              {busy?.startsWith("create:") ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Plus className="mr-1.5 h-3.5 w-3.5" />
              )}
              Create
            </Button>
          </div>
        </div>

        <div className="max-h-[50vh] overflow-y-auto">
          {branches == null && error == null && (
            <div className="flex items-center gap-2 px-5 py-6 text-sm text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Loading branches…
            </div>
          )}
          {error && (
            <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-2 text-xs text-destructive">
              {error}
            </div>
          )}
          {branches && sortedBranches.length === 0 && (
            <div className="px-5 py-8 text-center text-sm text-muted-foreground">
              No branches yet. Create one above.
            </div>
          )}
          {branches && sortedBranches.length > 0 && (
            <ul className="divide-y">
              {sortedBranches.map((b) => (
                <BranchRowView
                  key={b.name}
                  branch={b}
                  busy={busy}
                  onCheckout={() => void checkoutBranch(b.name)}
                  onMerge={() => void mergeBranch(b.name)}
                />
              ))}
            </ul>
          )}
        </div>

        <div className="flex items-center justify-end border-t bg-muted/30 px-5 py-3">
          <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

interface BranchRowProps {
  branch: BranchRow;
  busy: string | null;
  onCheckout: () => void;
  onMerge: () => void;
}

function BranchRowView({ branch, busy, onCheckout, onMerge }: BranchRowProps) {
  const isMain = branch.name === "main";
  const isBusy = busy === `merge:${branch.name}`;
  const Icon: LucideIcon = isMain ? GitBranch : GitBranch;
  return (
    <li className="flex items-center gap-3 px-5 py-3">
      <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md border bg-muted/40 text-muted-foreground">
        <Icon className="h-3.5 w-3.5" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium">{branch.name}</span>
          {isMain && (
            <span className="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-primary">
              main
            </span>
          )}
          {branch.current && (
            <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-emerald-600">
              current
            </span>
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-xs text-muted-foreground">
          <span>
            {branch.revisions} revision{branch.revisions === 1 ? "" : "s"}
          </span>
          {branch.head && (
            <>
              <span aria-hidden>·</span>
              <code className="font-mono">{branch.head}</code>
            </>
          )}
        </div>
      </div>
      <Button size="sm" variant="outline" onClick={onCheckout} disabled={busy !== null || branch.current}>
        {busy === `checkout:${branch.name}` && <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />}
        {branch.current ? "Checked out" : "Checkout"}
      </Button>
      {!isMain && (
        <Button
          size="sm"
          variant="outline"
          onClick={onMerge}
          disabled={isBusy || busy !== null}
          className={cn(isBusy && "opacity-60")}
          aria-label={`Merge ${branch.name} into main`}
        >
          {isBusy ? (
            <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
          ) : (
            <GitMerge className="mr-1.5 h-3.5 w-3.5" />
          )}
          Merge
        </Button>
      )}
    </li>
  );
}
