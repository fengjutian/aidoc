import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Clock,
  Copy,
  Download,
  ExternalLink,
  Eye,
  FilePlus,
  FileText,
  FolderOpen,
  Hash,
  Plus,
  Save,
  Sparkles,
  Trash2,
  Undo2,
} from "lucide-react";

import { CommandPalette } from "@/components/CommandPalette";
import { RevisionDiff } from "@/components/RevisionDiff";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { NodeEditor } from "@/NodeEditor";

interface Info {
  doc_id: string;
  title: string;
  head_revision: string;
  entry: string;
}

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

const kindIcon = (kind: string) => {
  // All AIDoc node kinds render with FileText by default; surface a few
  // distinguishing glyphs without coupling to the full kind taxonomy.
  if (kind === "section" || kind === "heading") return FileText;
  return FileText;
};

export default function App() {
  const [info, setInfo] = useState<Info | null>(null);
  const [nodes, setNodes] = useState<NodeRow[]>([]);
  const [revs, setRevs] = useState<RevisionRow[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [initPath, setInitPath] = useState("");
  const [title, setTitle] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [diffRev, setDiffRev] = useState<string | null>(null);

  const refresh = async () => {
    if (!info) return;
    try {
      const [n, r] = await Promise.all([
        invoke<NodeRow[]>("list_nodes"),
        invoke<RevisionRow[]>("list_revisions"),
      ]);
      setNodes(n);
      setRevs(r);
      setInfo((prev) =>
        prev ? { ...prev, head_revision: r.at(-1)?.id ?? prev.head_revision } : prev,
      );
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [info?.doc_id]);

  // Global keyboard shortcuts: ⌘K palette, ⌘S save, ⌘E export, ⌘N new node.
  // Skip when focus is inside an editable field so the OS / Radix can still
  // handle native text input.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      const editable =
        tag === "input" ||
        tag === "textarea" ||
        target?.isContentEditable === true;
      // ⌘K should always work even inside inputs (palette jumps to a search
      // input anyway). Other shortcuts defer to native text handling.
      const k = e.key.toLowerCase();
      if (k === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
        return;
      }
      if (editable) return;
      if (k === "s") {
        e.preventDefault();
        void onSave();
      } else if (k === "e") {
        e.preventDefault();
        void onExportHtml();
      } else if (k === "n") {
        e.preventDefault();
        const next = `node-${nodes.length + 1}`;
        void onCreateNode(next, "section", "");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [nodes.length]);

  const onInit = async () => {
    setError(null);
    try {
      const i = await invoke<Info>("init_doc", {
        path: initPath,
        docId: title.toLowerCase().replace(/\s+/g, "-") || "demo",
        title: title || "Untitled",
      });
      setInfo(i);
    } catch (e) {
      setError(String(e));
    }
  };

  const onOpen = async () => {
    setError(null);
    try {
      const i = await invoke<Info>("open_doc", { path: initPath });
      setInfo(i);
    } catch (e) {
      setError(String(e));
    }
  };

  const onUpdate = async (target: string, html: string) => {
    try {
      await invoke<string>("update_node", { target, content: html });
    } catch (e) {
      setError(String(e));
    }
  };

  const onCreateNode = async (id: string, kind: string, content: string) => {
    setError(null);
    try {
      await invoke<string>("create_node", { id, kind, content });
      setActiveId(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onDeleteNode = async (target: string) => {
    setError(null);
    try {
      await invoke<string>("delete_node", { target });
      if (activeId === target) setActiveId(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onCopyId = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id);
    } catch (e) {
      setError(String(e));
    }
  };

  const onRevert = async (revId: string) => {
    setError(null);
    try {
      await invoke<string>("revert", { target: revId });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onSave = async () => {
    setError(null);
    try {
      await invoke("save_doc");
    } catch (e) {
      setError(String(e));
    }
  };

  const onExportHtml = async () => {
    setError(null);
    try {
      const html = await invoke<string>("export_html");
      const blob = new Blob([html], { type: "text/html" });
      const url = URL.createObjectURL(blob);
      window.open(url, "_blank");
    } catch (e) {
      setError(String(e));
    }
  };

  if (!info) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-6 bg-background p-8">
        <div className="flex items-center gap-3">
          <Sparkles className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-semibold tracking-tight">AIDoc Desktop</h1>
        </div>
        <p className="max-w-md text-center text-sm text-muted-foreground">
          No document open. Initialize a new <code className="rounded bg-muted px-1.5 py-0.5">.aidoc</code>{" "}
          or open an existing one.
        </p>
        <div className="flex w-full max-w-xl flex-col gap-3 rounded-lg border bg-card p-4 shadow-sm">
          <input
            className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
            placeholder="examples/demo.aidoc"
            value={initPath}
            onChange={(e) => setInitPath(e.target.value)}
          />
          <input
            className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
            placeholder="Document title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <div className="flex gap-2">
            <Button onClick={onInit} disabled={!initPath} className="flex-1">
              <FilePlus className="mr-2 h-4 w-4" />
              Init
            </Button>
            <Button
              onClick={onOpen}
              disabled={!initPath}
              variant="outline"
              className="flex-1"
            >
              <FolderOpen className="mr-2 h-4 w-4" />
              Open
            </Button>
          </div>
        </div>
        {error && (
          <div className="flex items-center gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            <AlertCircle className="h-4 w-4" />
            {error}
          </div>
        )}
      </div>
    );
  }

  const active = nodes.find((n) => n.id === activeId);

  return (
    <TooltipProvider delayDuration={300}>
      <header className="flex h-12 shrink-0 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur">
        <Sparkles className="h-4 w-4 text-primary" />
        <h1 className="mr-auto text-sm font-semibold">
          {info.title}
          <span className="ml-2 font-normal text-muted-foreground">
            · head={info.head_revision}
          </span>
        </h1>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setPaletteOpen(true)}
              className="text-muted-foreground"
            >
              <span className="font-mono text-xs">⌘K</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>Open command palette</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button size="sm" variant="outline" onClick={onSave}>
              <Save className="mr-1.5 h-3.5 w-3.5" />
              Save
            </Button>
          </TooltipTrigger>
          <TooltipContent>Persist current state to .aidoc</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button size="sm" variant="outline" onClick={onExportHtml}>
              <Download className="mr-1.5 h-3.5 w-3.5" />
              Export HTML
            </Button>
          </TooltipTrigger>
          <TooltipContent>Render document to HTML and open in browser</TooltipContent>
        </Tooltip>
      </header>

      {error && (
        <div className="flex items-center gap-2 border-b border-destructive/40 bg-destructive/10 px-4 py-2 text-sm text-destructive">
          <AlertCircle className="h-4 w-4" />
          {error}
        </div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-[280px_1fr]">
        <aside className="flex min-h-0 flex-col border-r bg-muted/30">
          <ScrollArea className="flex-1">
            <div className="p-3">
              <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                <FileText className="h-3.5 w-3.5" />
                Nodes
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      size="icon"
                      variant="ghost"
                      className="ml-auto h-5 w-5"
                      onClick={() => {
                        const next = `node-${nodes.length + 1}`;
                        void onCreateNode(next, "section", "");
                      }}
                    >
                      <Plus className="h-3.5 w-3.5" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Add new section node</TooltipContent>
                </Tooltip>
              </div>
              <ul className="space-y-0.5">
                {nodes
                  .slice()
                  .sort((a, b) => a.position - b.position)
                  .map((n) => {
                    const Icon = kindIcon(n.kind);
                    return (
                      <li key={n.id}>
                        <ContextMenu>
                          <ContextMenuTrigger asChild>
                            <button
                              onClick={() => setActiveId(n.id)}
                              className={cn(
                                "flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm transition-colors hover:bg-accent",
                                n.id === activeId &&
                                  "bg-accent font-medium text-accent-foreground",
                              )}
                            >
                              <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                              <span className="truncate">{n.id}</span>
                              <span className="ml-auto text-[10px] uppercase text-muted-foreground">
                                {n.kind}
                              </span>
                            </button>
                          </ContextMenuTrigger>
                          <ContextMenuContent className="w-48">
                            <ContextMenuItem
                              onSelect={() => {
                                setActiveId(n.id);
                              }}
                            >
                              <ExternalLink className="text-muted-foreground" />
                              Open in editor
                            </ContextMenuItem>
                            <ContextMenuItem
                              onSelect={() => {
                                void onCopyId(n.id);
                              }}
                            >
                              <Copy className="text-muted-foreground" />
                              Copy node ID
                            </ContextMenuItem>
                            <ContextMenuSeparator />
                            <ContextMenuItem
                              disabled={n.id === "root"}
                              onSelect={() => {
                                void onDeleteNode(n.id);
                              }}
                              className="text-destructive focus:text-destructive"
                            >
                              <Trash2 />
                              Delete node
                            </ContextMenuItem>
                          </ContextMenuContent>
                        </ContextMenu>
                      </li>
                    );
                  })}
              </ul>

              <Separator className="my-3" />

              <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                <Clock className="h-3.5 w-3.5" />
                History
              </div>
              <ol className="space-y-1">
                {revs.map((r) => {
                  const isHead = r.id === info.head_revision;
                  return (
                    <li
                      key={r.id}
                      className="flex items-center gap-1 rounded-md px-2 py-1 text-xs hover:bg-accent"
                    >
                      <Hash className="h-3 w-3 shrink-0 text-muted-foreground" />
                      <code className="font-mono">{r.id}</code>
                      <span className="truncate text-muted-foreground">
                        {r.message ?? ""}
                      </span>
                      <div className="ml-auto flex items-center gap-0.5">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              size="icon"
                              variant="ghost"
                              className="h-6 w-6"
                              onClick={() => setDiffRev(r.id)}
                            >
                              <Eye className="h-3 w-3" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>View diff</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              size="icon"
                              variant="ghost"
                              className="h-6 w-6"
                              onClick={() => onRevert(r.id)}
                              disabled={isHead}
                            >
                              <Undo2 className="h-3 w-3" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {isHead ? "Already at head" : "Revert to this revision"}
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    </li>
                  );
                })}
              </ol>
            </div>
          </ScrollArea>
        </aside>

        <main className="flex min-h-0 flex-col overflow-hidden">
          <div className="flex-1 overflow-auto p-6">
            {active ? (
              <NodeEditor
                key={active.id}
                kind={active.kind as never}
                content={active.content}
                onChange={(html) => onUpdate(active.id, html)}
              />
            ) : (
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                Select a node from the tree to edit.
              </div>
            )}
          </div>
        </main>
      </div>

      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        nodes={nodes}
        revs={revs}
        info={info}
        onSelectNode={(id) => {
          setActiveId(id);
        }}
        onSave={() => {
          void onSave();
        }}
        onExportHtml={() => {
          void onExportHtml();
        }}
        onRevert={(id) => {
          void onRevert(id);
        }}
        onCreateNode={(id, kind, content) => {
          void onCreateNode(id, kind, content);
        }}
        onDeleteNode={(target) => {
          void onDeleteNode(target);
        }}
      />

      <RevisionDiff
        revId={diffRev}
        onClose={() => {
          setDiffRev(null);
        }}
      />
    </TooltipProvider>
  );
}