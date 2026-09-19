import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  Clock,
  Download,
  Eye,
  FilePlus,
  FileText,
  FolderOpen,
  FolderSearch,
  Hash,
  Plus,
  Save,
  Settings as SettingsIcon,
  Sparkles,
  Undo2,
  Search as SearchIcon,
  X as XIcon,
} from "lucide-react";

import { AiChat } from "@/components/AiChat";
import { AttributesDialog } from "@/components/AttributesDialog";
import { BranchDialog } from "@/components/BranchDialog";
import { CommandPalette } from "@/components/CommandPalette";
import { LinkDialog } from "@/components/LinkDialog";
import { RevisionDiff } from "@/components/RevisionDiff";
import { SaveStatus } from "@/components/SaveStatus";
import { SettingsPanel } from "@/components/SettingsPanel";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { useRecentFiles } from "@/hooks/useRecentFiles";
import { useSettings } from "@/hooks/useSettings";
import { useTheme } from "@/hooks/useTheme";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
import { NodeTree } from "@/NodeTree";

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
  attributes?: Record<string, string>;
}

interface RevisionRow {
  id: string;
  parent: string | null;
  operation: string;
  created_at: string;
  message: string | null;
}

interface RelationRow {
  id: string;
  source: string;
  target: string;
  kind: string;
}

export default function App() {
  const theme = useTheme();
  const { settings, update: updateSettings } = useSettings();
  const { recent, addOpened, remove: removeRecent } = useRecentFiles();
  const [info, setInfo] = useState<Info | null>(null);
  const [nodes, setNodes] = useState<NodeRow[]>([]);
  const [revs, setRevs] = useState<RevisionRow[]>([]);
  const [relations, setRelations] = useState<RelationRow[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [initPath, setInitPath] = useState("");
  const [title, setTitle] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [diffRev, setDiffRev] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [aiHistory, setAiHistory] = useState<
    { role: "user" | "assistant" | "error"; content: string }[]
  >([]);
  const [branchOpen, setBranchOpen] = useState(false);
  const [linkSource, setLinkSource] = useState<string | null>(null);
  const [attrsNodeId, setAttrsNodeId] = useState<string | null>(null);

  // Debounced AI-history persistence. Whenever `aiHistory` changes, schedule
  // a write to the `__ai_history__` metadata node — coalesced so a fast back-
  // and-forth conversation only produces one revision.
  const aiHistoryTimer = useRef<number | null>(null);
  useEffect(() => {
    if (!info) return;
    if (aiHistoryTimer.current !== null) {
      window.clearTimeout(aiHistoryTimer.current);
    }
    aiHistoryTimer.current = window.setTimeout(async () => {
      try {
        const json = JSON.stringify(aiHistory);
        // Always create-or-update via attributes so we don't fight the nodes
        // list ordering. set_node_attributes accepts both an existing target
        // and will create-then-update on the first run via create_node.
        const exists = nodes.some((n) => n.id === "__ai_history__");
        if (exists) {
          await invoke<string>("set_node_attributes", {
            target: "__ai_history__",
            attrs: { history: json },
          });
        } else {
          await invoke<string>("create_node", {
            id: "__ai_history__",
            kind: "generic",
            content: "",
          });
          await invoke<string>("set_node_attributes", {
            target: "__ai_history__",
            attrs: { history: json },
          });
        }
      } catch {
        // Silently swallow — the user will notice their next send fails
        // if the underlying issue is structural, and persistence is best-
        // effort.
      }
    }, 400);
    return () => {
      if (aiHistoryTimer.current !== null) {
        window.clearTimeout(aiHistoryTimer.current);
      }
    };
  }, [aiHistory, info, nodes]);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [docPath, setDocPath] = useState<string | null>(null);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "dirty" | "never">("never");
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<NodeRow[] | null>(null);

  const refresh = async () => {
    if (!info) return;
    try {
      const [n, r, rel] = await Promise.all([
        invoke<NodeRow[]>("list_nodes"),
        invoke<RevisionRow[]>("list_revisions"),
        invoke<RelationRow[]>("list_relations"),
      ]);
      setNodes(n);
      setRevs(r);
      setRelations(rel);
      // Load AI chat history from a dedicated metadata node.
      const histNode = n.find((x) => x.id === "__ai_history__");
      if (histNode?.attributes?.history) {
        try {
          const parsed = JSON.parse(histNode.attributes.history);
          if (Array.isArray(parsed)) {
            setAiHistory(
              parsed.filter(
                (t: unknown): t is { role: "user" | "assistant" | "error"; content: string } =>
                  typeof t === "object" &&
                  t !== null &&
                  typeof (t as { role?: unknown }).role === "string" &&
                  ["user", "assistant", "error"].includes(
                    (t as { role: string }).role,
                  ) &&
                  typeof (t as { content?: unknown }).content === "string",
              ),
            );
          } else {
            setAiHistory([]);
          }
        } catch {
          setAiHistory([]);
        }
      } else {
        setAiHistory([]);
      }
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

  // Auto-restore the most-recently-opened .aidoc on first paint, if the file
  // still exists. Failed attempts (missing / moved files) fall through to
  // the welcome screen with no error — user can pick from the Recent list.
  const autoOpenAttempted = useRef(false);
  useEffect(() => {
    if (autoOpenAttempted.current) return;
    autoOpenAttempted.current = true;
    const last = recent[0];
    if (!last) return;
    void (async () => {
      try {
        const i = await invoke<Info>("open_doc", { path: last });
        setInfo(i);
        setDocPath(last);
      } catch {
        // File moved / deleted. Drop it from recents silently.
        removeRecent(last);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Apply editor font size to a CSS var the stylesheet reads.
  useEffect(() => {
    const px = settings.fontSize === "sm" ? "0.85rem" : settings.fontSize === "lg" ? "1.05rem" : "0.95rem";
    document.documentElement.style.setProperty("--editor-font-size", px);
  }, [settings.fontSize]);

  // Debounced node full-text search. Backend exposes `search_nodes` (FTS5
  // over the SQLite doc db). Empty query → null and the sidebar falls back
  // to the full nodes list.
  useEffect(() => {
    if (!info) {
      setSearchResults(null);
      return;
    }
    const q = searchQuery.trim();
    if (!q) {
      setSearchResults(null);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(async () => {
      try {
        const hits = await invoke<NodeRow[]>("search_nodes", { query: q });
        if (!cancelled) setSearchResults(hits);
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setSearchResults(null);
        }
      }
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [searchQuery, info]);

  // Global keyboard shortcuts: ⌘K palette, ⌘S save, ⌘E export HTML,
  // ⌘⇧E export Markdown, ⌘N new node. Skip when focus is inside an editable
  // field so the OS / Radix can still handle native text input.
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
        if (e.shiftKey) void onExportMarkdown();
        else void onExportHtml();
      } else if (k === "n") {
        e.preventDefault();
        const next = `node-${nodes.length + 1}`;
        void onCreateNode(next, "section", "");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [nodes.length]);

  // Native file picker — populates the path field on the welcome screen so
  // users don't have to hand-type a Windows path. Falls back to whatever
  // they typed if the dialog is cancelled.
  const pickOpenPath = async (): Promise<string | null> => {
    try {
      const picked = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "AIDoc", extensions: ["aidoc"] }],
      });
      return typeof picked === "string" ? picked : null;
    } catch (e) {
      setError(String(e));
      return null;
    }
  };

  const pickSavePath = async (defaultName: string): Promise<string | null> => {
    try {
      const picked = await saveDialog({
        defaultPath: defaultName,
        filters: [{ name: "AIDoc", extensions: ["aidoc"] }],
      });
      return picked ?? null;
    } catch (e) {
      setError(String(e));
      return null;
    }
  };

  const openFromDialog = async () => {
    setError(null);
    const picked = await pickOpenPath();
    if (!picked) return;
    await onOpen(picked);
  };

  const initFromDialog = async () => {
    setError(null);
    const defaultName =
      (title || "untitled").toLowerCase().replace(/\s+/g, "-") + ".aidoc";
    const picked = await pickSavePath(defaultName);
    if (!picked) return;
    await onInit(picked);
  };

  const onInit = async (path: string) => {
    setError(null);
    try {
      const i = await invoke<Info>("init_doc", {
        path,
        docId: title.toLowerCase().replace(/\s+/g, "-") || "demo",
        title: title || "Untitled",
      });
      setInfo(i);
      setDocPath(path);
      addOpened(path);
    } catch (e) {
      setError(String(e));
    }
  };

  const onOpen = async (path: string) => {
    setError(null);
    try {
      const i = await invoke<Info>("open_doc", { path });
      setInfo(i);
      setDocPath(path);
      addOpened(path);
    } catch (e) {
      setError(String(e));
    }
  };

  const onUpdate = async (target: string, html: string) => {
    try {
      await invoke<string>("update_node", { target, content: html });
      if (settings.autosave) {
        setSaveState("saving");
        await invoke("save_doc");
        setLastSavedAt(new Date());
        setSaveState("saved");
      } else {
        setSaveState("dirty");
      }
    } catch (e) {
      setError(String(e));
      setSaveState("idle");
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

  const onChangeKind = async (target: string, kind: string) => {
    setError(null);
    try {
      await invoke<string>("set_node_kind", { target, kind });
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
    setSaveState("saving");
    try {
      await invoke("save_doc");
      setLastSavedAt(new Date());
      setSaveState("saved");
    } catch (e) {
      setError(String(e));
      setSaveState("idle");
    }
  };

  const onSaveAs = async () => {
    setError(null);
    const defaultName =
      (info?.title || "untitled").toLowerCase().replace(/\s+/g, "-") + ".aidoc";
    const picked = await pickSavePath(defaultName);
    if (!picked) return;
    setSaveState("saving");
    try {
      await invoke("save_doc_as", { path: picked });
      setDocPath(picked);
      addOpened(picked);
      setLastSavedAt(new Date());
      setSaveState("saved");
    } catch (e) {
      setError(String(e));
      setSaveState("idle");
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

  const onExportMarkdown = async () => {
    setError(null);
    try {
      const md = await invoke<string>("export_markdown");
      const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${info?.title || "untitled"}.md`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(String(e));
    }
  };

  if (!info) {
    return (
      <div className="relative flex h-full flex-col items-center justify-center gap-6 bg-background p-8">
        <div className="absolute right-4 top-4">
          <ThemeToggle
            theme={theme.theme}
            resolved={theme.resolved}
            onSet={theme.set}
          />
        </div>
        <div className="flex items-center gap-3">
          <Sparkles className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-semibold tracking-tight">AIDoc Desktop</h1>
        </div>
        <p className="max-w-md text-center text-sm text-muted-foreground">
          No document open. Initialize a new <code className="rounded bg-muted px-1.5 py-0.5">.aidoc</code>{" "}
          or open an existing one.
        </p>

        {recent.length > 0 && (
          <div className="flex w-full max-w-xl flex-col gap-1.5 rounded-md border bg-card/60 p-3">
            <div className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              <span>Recent</span>
              <span className="text-muted-foreground/60">{recent.length}</span>
            </div>
            <ul className="space-y-0.5">
              {recent.slice(0, 5).map((p) => (
                <li key={p} className="group flex items-center gap-1 rounded-md hover:bg-accent">
                  <button
                    type="button"
                    onClick={() => void onOpen(p)}
                    className="flex flex-1 items-center gap-2 truncate rounded-md px-2 py-1 text-left text-sm"
                  >
                    <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                    <span className="truncate font-mono text-xs">{p}</span>
                  </button>
                  <button
                    type="button"
                    aria-label={`Forget ${p}`}
                    className="mr-1 inline-flex h-6 w-6 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                    onClick={() => removeRecent(p)}
                  >
                    <XIcon className="h-3 w-3" />
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="flex w-full max-w-xl flex-col gap-3 rounded-lg border bg-card p-4 shadow-sm">
          <div className="flex gap-2">
            <input
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
              placeholder="examples/demo.aidoc"
              value={initPath}
              onChange={(e) => setInitPath(e.target.value)}
            />
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-9 w-9 shrink-0"
              aria-label="Browse for .aidoc file"
              onClick={async () => {
                const picked = await pickOpenPath();
                if (picked) setInitPath(picked);
              }}
            >
              <FolderSearch className="h-4 w-4" />
            </Button>
          </div>
          <input
            className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
            placeholder="Document title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <div className="flex gap-2">
            <Button onClick={initFromDialog} className="flex-1">
              <FilePlus className="mr-2 h-4 w-4" />
              New document…
            </Button>
            <Button onClick={openFromDialog} variant="outline" className="flex-1">
              <FolderOpen className="mr-2 h-4 w-4" />
              Open file…
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

        <SaveStatus state={saveState} lastSavedAt={lastSavedAt} />

        <ThemeToggle
          theme={theme.theme}
          resolved={theme.resolved}
          onSet={theme.set}
        />

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="icon"
              variant="ghost"
              className="h-8 w-8"
              onClick={() => setAiOpen(true)}
            >
              <Sparkles className="h-4 w-4 text-primary" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Ask AIDoc AI</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="icon"
              variant="ghost"
              className="h-8 w-8"
              onClick={() => setSettingsOpen(true)}
            >
              <SettingsIcon className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Settings</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="sm"
              variant="ghost"
              className="h-8 px-2 text-muted-foreground"
              onClick={openFromDialog}
            >
              <FolderOpen className="mr-1.5 h-3.5 w-3.5" />
              Open
            </Button>
          </TooltipTrigger>
          <TooltipContent>Open a different .aidoc file</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="sm"
              variant="ghost"
              className="h-8 px-2 text-muted-foreground"
              onClick={async () => {
                const defaultName =
                  (info?.title || "untitled").toLowerCase().replace(/\s+/g, "-") + ".aidoc";
                const picked = await pickSavePath(defaultName);
                if (!picked) return;
                setError(null);
                try {
                  const i = await invoke<Info>("init_doc", {
                    path: picked,
                    docId: info?.doc_id ?? "demo",
                    title: info?.title ?? "Untitled",
                  });
                  setInfo(i);
                  setDocPath(picked);
                } catch (e) {
                  setError(String(e));
                }
              }}
            >
              <FilePlus className="mr-1.5 h-3.5 w-3.5" />
              New
            </Button>
          </TooltipTrigger>
          <TooltipContent>Create a new .aidoc file (current doc will be discarded)</TooltipContent>
        </Tooltip>

        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button size="sm" variant="outline">
                  <Save className="mr-1.5 h-3.5 w-3.5" />
                  Save
                  <span className="ml-1 text-xs text-muted-foreground">▾</span>
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>Save (⇧ for Save As)</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => void onSave()}>
              <Save className="text-muted-foreground" />
              Save
              <span className="ml-auto text-xs text-muted-foreground">⌘ S</span>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void onSaveAs()}>
              <FilePlus className="text-muted-foreground" />
              Save As…
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button size="sm" variant="outline" onClick={onExportHtml}>
              <Download className="mr-1.5 h-3.5 w-3.5" />
              Export HTML
            </Button>
          </TooltipTrigger>
          <TooltipContent>Render document to HTML and open in browser</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button size="sm" variant="outline" onClick={onExportMarkdown}>
              <Download className="mr-1.5 h-3.5 w-3.5" />
              Export MD
            </Button>
          </TooltipTrigger>
          <TooltipContent>Render document as Markdown and save as .md</TooltipContent>
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
          <div className="border-b p-2">
            <div className="relative">
              <SearchIcon className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <input
                className="h-8 w-full rounded-md border border-input bg-background pl-7 pr-7 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
                placeholder="Search nodes…"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") setSearchQuery("");
                }}
              />
              {searchQuery && (
                <button
                  type="button"
                  aria-label="Clear search"
                  className="absolute right-1 top-1/2 inline-flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                  onClick={() => setSearchQuery("")}
                >
                  <XIcon className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          </div>
          <ScrollArea className="flex-1">
            <div className="p-3">
              {searchQuery.trim() && searchResults ? (
                <>
                  <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <SearchIcon className="h-3.5 w-3.5" />
                    Results ({searchResults.length})
                  </div>
                  {searchResults.length === 0 ? (
                    <div className="rounded-md border border-dashed bg-background/50 px-3 py-6 text-center text-xs text-muted-foreground">
                      No node matches &ldquo;{searchQuery.trim()}&rdquo;.
                    </div>
                  ) : (
                    <ul className="space-y-0.5">
                      {searchResults.map((n) => (
                        <li key={n.id}>
                          <button
                            type="button"
                            onClick={() => setActiveId(n.id)}
                            className={cn(
                              "flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm transition-colors hover:bg-accent",
                              n.id === activeId &&
                                "bg-accent font-medium text-accent-foreground",
                            )}
                          >
                            <FileText className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                            <span className="truncate">{n.id}</span>
                            <span className="ml-auto text-[10px] uppercase text-muted-foreground">
                              {n.kind}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </>
              ) : (
                <>
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
              <NodeTree
                nodes={nodes}
                activeId={activeId}
                collapsed={collapsed}
                setCollapsed={setCollapsed}
                setActiveId={setActiveId}
                setLinkSource={setLinkSource}
                setAttrsNodeId={setAttrsNodeId}
                onCopyId={onCopyId}
                onReparent={(target, newParent) => {
                  setError(null);
                  void (async () => {
                    try {
                      await invoke<string>("move_node", {
                        target,
                        newPosition: 0,
                        newParent,
                      });
                      await refresh();
                    } catch (e) {
                      setError(String(e));
                    }
                  })();
                }}
                onDeleteNode={onDeleteNode}
                dragOverId={dragOverId}
                setDragOverId={setDragOverId}
              />
                </>
              )}

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
                onKindChange={(kind) => onChangeKind(active.id, kind)}
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
        onExportMarkdown={() => {
          void onExportMarkdown();
        }}
        onOpenBranches={() => setBranchOpen(true)}
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
        revs={revs}
        headRevision={info?.head_revision ?? null}
      />

      <SettingsPanel
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        settings={settings}
        onUpdate={updateSettings}
      />

      <AiChat
        open={aiOpen}
        onOpenChange={setAiOpen}
        settings={settings}
        docPath={docPath}
        nodeIds={nodes.map((n) => n.id)}
        onJumpToNode={(id) => {
          setActiveId(id);
          setAiOpen(false);
        }}
        initialHistory={aiHistory}
        onHistoryChange={setAiHistory}
      />

      <BranchDialog
        open={branchOpen}
        onOpenChange={setBranchOpen}
        onChanged={() => {
          void refresh();
        }}
      />

      <LinkDialog
        open={linkSource !== null}
        onOpenChange={(o) => {
          if (!o) setLinkSource(null);
        }}
        source={linkSource}
        nodes={nodes}
        relations={relations}
        onChanged={() => {
          void refresh();
        }}
      />

      <AttributesDialog
        open={attrsNodeId !== null}
        onOpenChange={(o) => {
          if (!o) setAttrsNodeId(null);
        }}
        nodeId={attrsNodeId}
        initial={
          (attrsNodeId &&
            nodes.find((n) => n.id === attrsNodeId)?.attributes) ||
          {}
        }
        onSaved={() => {
          void refresh();
        }}
      />
    </TooltipProvider>
  );
}