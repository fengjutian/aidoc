import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowRight,
  CheckCircle2,
  Download,
  FilePlus,
  FileText,
  GitBranch,
  GitMerge,
  Save,
  ShieldCheck,
  Trash2,
  Undo2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";

interface NodeRow {
  id: string;
  kind: string;
  parent: string | null;
  position: number;
  content: string;
}

interface RevisionRow {
  id: string;
  parent: string | null;
  operation: string;
  created_at: string;
  message: string | null;
}

interface Info {
  head_revision: string;
  title: string;
}

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  nodes: NodeRow[];
  revs: RevisionRow[];
  info: Info | null;
  onSelectNode: (id: string) => void;
  onSave: () => void;
  onExportHtml: () => void;
  onRevert: (revId: string) => void;
  onCreateNode: (id: string, kind: string, content: string) => void;
  onDeleteNode: (target: string) => void;
}

interface Action {
  id: string;
  label: string;
  group: "Document" | "Revisions";
  icon: LucideIcon;
  shortcut?: string;
  disabled?: boolean;
  run: () => void;
}

export function CommandPalette({
  open,
  onOpenChange,
  nodes,
  revs,
  info,
  onSelectNode,
  onSave,
  onExportHtml,
  onRevert,
  onCreateNode,
  onDeleteNode,
}: CommandPaletteProps) {
  const [validationStatus, setValidationStatus] = useState<
    "idle" | "running" | "passed" | "failed"
  >("idle");

  useEffect(() => {
    // Reset transient state when palette closes.
    if (!open) setValidationStatus("idle");
  }, [open]);

  const runValidate = async () => {
    setValidationStatus("running");
    try {
      await invoke<string>("validate_aidoc");
      setValidationStatus("passed");
    } catch (e) {
      setValidationStatus("failed");
      // eslint-disable-next-line no-console
      console.error("validate failed:", e);
    }
  };

  const docActions: Action[] = [
    {
      id: "save",
      label: "Save document",
      group: "Document",
      icon: Save,
      shortcut: "⌘ S",
      run: () => {
        onSave();
        onOpenChange(false);
      },
    },
    {
      id: "export-html",
      label: "Export HTML",
      group: "Document",
      icon: Download,
      shortcut: "⌘ E",
      run: () => {
        onExportHtml();
        onOpenChange(false);
      },
    },
    {
      id: "validate",
      label:
        validationStatus === "running"
          ? "Validating…"
          : validationStatus === "passed"
            ? "Validate (last run: passed)"
            : validationStatus === "failed"
              ? "Validate (last run: failed)"
              : "Validate document",
      group: "Document",
      icon:
        validationStatus === "passed"
          ? CheckCircle2
          : validationStatus === "failed"
            ? CheckCircle2
            : ShieldCheck,
      shortcut: "⌘ ⇧ V",
      disabled: validationStatus === "running",
      run: () => {
        void runValidate();
      },
    },
    {
      id: "branch",
      label: "Branch current head",
      group: "Document",
      icon: GitBranch,
      disabled: true,
      run: () => {},
    },
    {
      id: "merge",
      label: "Merge branch into main",
      group: "Document",
      icon: GitMerge,
      disabled: true,
      run: () => {},
    },
    {
      id: "new-node",
      label: "New section node",
      group: "Document",
      icon: FilePlus,
      shortcut: "⌘ N",
      run: () => {
        const next = `node-${nodes.length + 1}`;
        onCreateNode(next, "section", "");
        onOpenChange(false);
      },
    },
  ];

  const sortedNodes = [...nodes].sort((a, b) => a.position - b.position);
  const sortedRevs = [...revs].reverse(); // newest first
  const isHead = (id: string) => info?.head_revision === id;

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} className="max-w-2xl">
      <CommandInput placeholder="Type a command or search a node…" />
      <CommandList>
        <CommandEmpty>No results.</CommandEmpty>

        <CommandGroup heading="Document">
          {docActions.map((a) => (
            <CommandItem
              key={a.id}
              value={`${a.label} ${a.id}`}
              disabled={a.disabled}
              onSelect={a.run}
            >
              <a.icon
                className={
                  a.id === "validate" && validationStatus === "failed"
                    ? "text-destructive"
                    : a.id === "validate" && validationStatus === "passed"
                      ? "text-emerald-500"
                      : "text-muted-foreground"
                }
              />
              <span>{a.label}</span>
              {a.shortcut && <CommandShortcut>{a.shortcut}</CommandShortcut>}
              {a.disabled && (
                <span className="ml-auto text-xs text-muted-foreground">soon</span>
              )}
            </CommandItem>
          ))}
        </CommandGroup>

        <CommandSeparator />

        <CommandGroup heading={`Nodes (${sortedNodes.length})`}>
          {sortedNodes.map((n) => (
            <CommandItem
              key={n.id}
              value={`node ${n.id} ${n.kind} ${n.content} delete`}
              onSelect={() => {
                onSelectNode(n.id);
                onOpenChange(false);
              }}
            >
              <FileText className="text-muted-foreground" />
              <span className="truncate">{n.id}</span>
              <span className="ml-1 text-[10px] uppercase text-muted-foreground">
                {n.kind}
              </span>
              <CommandShortcut>
                <ArrowRight className="h-3 w-3" />
              </CommandShortcut>
              {n.id !== "root" && (
                <span
                  role="button"
                  tabIndex={0}
                  className="ml-2 inline-flex h-6 w-6 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (
                      window.confirm(
                        `Delete node "${n.id}"? History is preserved.`,
                      )
                    ) {
                      onDeleteNode(n.id);
                      onOpenChange(false);
                    }
                  }}
                  aria-label={`Delete ${n.id}`}
                >
                  <Trash2 className="h-3 w-3" />
                </span>
              )}
            </CommandItem>
          ))}
        </CommandGroup>

        <CommandSeparator />

        <CommandGroup heading={`Revisions (${sortedRevs.length})`}>
          {sortedRevs.map((r) => (
            <CommandItem
              key={r.id}
              value={`revision ${r.id} ${r.message ?? ""}`}
              disabled={isHead(r.id)}
              onSelect={() => {
                onRevert(r.id);
                onOpenChange(false);
              }}
            >
              <Undo2 className="text-muted-foreground" />
              <span className="font-mono">{r.id}</span>
              <span className="truncate text-muted-foreground">
                {r.message ?? ""}
              </span>
              {isHead(r.id) ? (
                <CommandShortcut>head</CommandShortcut>
              ) : (
                <CommandShortcut>revert</CommandShortcut>
              )}
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}