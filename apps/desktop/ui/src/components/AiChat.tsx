import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Download, Eraser, Loader2, Send, Sparkles, Square } from "lucide-react";

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
import type { Settings } from "@/hooks/useSettings";

interface AiChatProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  settings: Settings;
  docPath: string | null;
  nodeIds: string[];
  onJumpToNode: (id: string) => void;
  /** Conversation turns loaded from the .aidoc package on open. */
  initialHistory: ChatTurn[];
  /** Called whenever the conversation turns change. Parent should persist
   *  these into the document via `set_node_attributes(target, { history })`. */
  onHistoryChange?: (turns: ChatTurn[]) => void;
}

interface ChatTurn {
  role: "user" | "assistant" | "error";
  content: string;
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function renderWithLinks(
  text: string,
  ids: string[],
  onJump: (id: string) => void,
): React.ReactNode {
  if (ids.length === 0) return text;
  const pattern = new RegExp(`\\b(${ids.map(escapeRegex).join("|")})\\b`, "g");
  const parts: React.ReactNode[] = [];
  let last = 0;
  let key = 0;
  for (const m of text.matchAll(pattern)) {
    if (m.index === undefined) continue;
    if (m.index > last) parts.push(text.slice(last, m.index));
    const id = m[0];
    parts.push(
      <button
        key={`lnk-${key++}`}
        type="button"
        onClick={() => onJump(id)}
        className="rounded bg-primary/10 px-1 font-mono text-xs text-primary hover:bg-primary/20 hover:underline"
      >
        {id}
      </button>,
    );
    last = m.index + id.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}

export function AiChat({
  open,
  onOpenChange,
  settings,
  docPath,
  nodeIds,
  onJumpToNode,
  initialHistory,
  onHistoryChange,
}: AiChatProps) {
  const [prompt, setPrompt] = useState("");
  const [history, setHistory] = useState<ChatTurn[]>(initialHistory);
  const [pending, setPending] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Re-seed history when the document changes (initialHistory is a fresh prop).
  useEffect(() => {
    setHistory(initialHistory);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [docPath]);

  // Bubble changes back to the parent for persistence into the .aidoc package.
  // Errors aren't worth persisting — strip them before saving.
  useEffect(() => {
    if (!onHistoryChange) return;
    const cleanable = history.filter((t) => t.role !== "error");
    onHistoryChange(cleanable);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [history]);
  const [suggestion, setSuggestion] = useState<{
    start: number;
    end: number;
    prefix: string;
    items: string[];
    index: number;
  } | null>(null);

  const sortedNodeIds = useMemo(
    () => [...nodeIds].sort((a, b) => a.localeCompare(b)),
    [nodeIds],
  );

  useEffect(() => {
    if (!open) return;
    setPrompt("");
    setSuggestion(null);
  }, [open]);

  // Recompute `@nodeId` autocomplete suggestions as the user types.
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) {
      setSuggestion(null);
      return;
    }
    const caret = ta.selectionStart ?? prompt.length;
    // Walk back from caret to find the most recent `@token` (no whitespace).
    const before = prompt.slice(0, caret);
    const m = /(^|\s)@([\w.-]*)$/.exec(before);
    if (!m) {
      setSuggestion(null);
      return;
    }
    const tokenStart = caret - m[2].length;
    const prefix = m[2].toLowerCase();
    const items = sortedNodeIds
      .filter((id) => id.toLowerCase().startsWith(prefix))
      .slice(0, 6);
    if (items.length === 0) {
      setSuggestion(null);
      return;
    }
    setSuggestion((prev) =>
      prev && prev.start === tokenStart && prev.prefix === m[2]
        ? { ...prev, items, index: Math.min(prev.index, items.length - 1) }
        : { start: tokenStart, end: caret, prefix: m[2], items, index: 0 },
    );
  }, [prompt, sortedNodeIds]);

  const applySuggestion = (id: string) => {
    if (!suggestion) return;
    const before = prompt.slice(0, suggestion.start);
    const after = prompt.slice(suggestion.end);
    const inserted = `@${id} `;
    const next = before + inserted + after;
    setPrompt(next);
    setSuggestion(null);
    // Restore caret right after the inserted node id.
    requestAnimationFrame(() => {
      const ta = textareaRef.current;
      if (!ta) return;
      const caret = before.length + inserted.length;
      ta.focus();
      ta.setSelectionRange(caret, caret);
    });
  };

  const moveSuggestion = (delta: number) => {
    if (!suggestion) return;
    const next =
      (suggestion.index + delta + suggestion.items.length) %
      suggestion.items.length;
    setSuggestion({ ...suggestion, index: next });
  };

  const clear = () => {
    setHistory([]);
    setPrompt("");
  };

  const exportConversation = (fmt: "md" | "json") => {
    const stamp = new Date().toISOString().slice(0, 16).replace("T", " ");
    const day = new Date().toISOString().slice(0, 10);
    if (fmt === "md") {
      const lines: string[] = [];
      lines.push(`# AIDoc AI Chat — ${stamp}`);
      if (docPath) lines.push(`\n_Document: \`${docPath}\`_\n`);
      for (const t of history) {
        if (t.role === "user") lines.push(`\n## You\n\n${t.content}\n`);
        else if (t.role === "assistant") lines.push(`\n## AIDoc AI\n\n${t.content}\n`);
        else lines.push(`\n## ⚠ Error\n\n\`\`\`\n${t.content}\n\`\`\`\n`);
      }
      const blob = new Blob([lines.join("")], { type: "text/markdown;charset=utf-8" });
      download(blob, `aidoc-chat-${day}.md`);
    } else {
      const payload = {
        exported_at: stamp,
        doc_path: docPath,
        turns: history,
      };
      const blob = new Blob([JSON.stringify(payload, null, 2)], {
        type: "application/json;charset=utf-8",
      });
      download(blob, `aidoc-chat-${day}.json`);
    }
  };

  const download = (blob: Blob, filename: string) => {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const unlistenersRef = useRef<UnlistenFn[]>([]);

  const cleanupListeners = () => {
    for (const u of unlistenersRef.current) u();
    unlistenersRef.current = [];
  };

  const abort = async () => {
    cleanupListeners();
    try {
      await invoke<boolean>("abort_ai_chat");
    } catch {
      /* ignore */
    }
  };

  const send = async () => {
    const text = prompt.trim();
    if (!text || pending) return;
    const prior = history;
    const next = [...prior, { role: "user" as const, content: text }];
    setHistory(next);
    setPrompt("");
    setPending(true);

    // Reserve a slot for the assistant turn; we'll mutate it as chunks arrive.
    const assistantIdx = next.length;
    setHistory((h) => [...h, { role: "assistant", content: "" }]);
    let acc = "";

    try {
      const chunkUn = await listen<string>("ai-chunk", (e) => {
        acc += e.payload;
        setHistory((h) => {
          if (assistantIdx >= h.length) return h;
          const copy = h.slice();
          copy[assistantIdx] = { role: "assistant", content: acc };
          return copy;
        });
      });
      const stderrUn = await listen<string>("ai-stderr", (e) => {
        // Stderr is surfaced as an error turn so users see what went wrong.
        cleanupListeners();
        setHistory((h) => [...h, { role: "error", content: e.payload }]);
        setPending(false);
      });
      const doneUn = await listen<boolean>("ai-done", () => {
        cleanupListeners();
        setHistory((h) => {
          if (assistantIdx >= h.length) return h;
          if (!acc) {
            const copy = h.slice();
            copy[assistantIdx] = { role: "assistant", content: "(empty response)" };
            return copy;
          }
          return h;
        });
        setPending(false);
      });
      const errorUn = await listen<string>("ai-error", (e) => {
        cleanupListeners();
        setHistory((h) => [...h, { role: "error", content: e.payload }]);
        setPending(false);
      });
      unlistenersRef.current = [chunkUn, stderrUn, doneUn, errorUn];

      await invoke("ai_chat", {
        prompt: text,
        apiKey: settings.openaiApiKey,
        baseUrl: settings.openaiBaseUrl,
        model: settings.openaiModel,
        docPath,
        history: prior,
      });
      // Command returns "started" immediately; ai-done/ai-error cleanup
      // listeners above flip `pending` to false.
    } catch (e) {
      cleanupListeners();
      setHistory((h) => [...h, { role: "error", content: String(e) }]);
    }
  };

  // Drop listeners when the dialog closes so we don't leak handlers.
  useEffect(() => {
    if (!open) {
      cleanupListeners();
      setPending(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl gap-0 p-0">
        <DialogHeader className="flex flex-row items-start justify-between gap-2 border-b px-5 py-4">
          <div className="min-w-0">
            <DialogTitle className="flex items-center gap-2">
              <Sparkles className="h-4 w-4 text-primary" />
              Ask AIDoc AI
            </DialogTitle>
            <DialogDescription>
              {docPath ? (
                <>
                  Reading <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]">{docPath}</code>
                </>
              ) : (
                <>Open a .aidoc document first; the agent will read it via MCP tools.</>
              )}
            </DialogDescription>
          </div>
          {history.length > 0 && (
            <div className="flex shrink-0 items-center gap-1">
              <Button
                size="sm"
                variant="ghost"
                className="h-7 px-2 text-xs text-muted-foreground"
                onClick={() => exportConversation("md")}
                aria-label="Export as Markdown"
              >
                <Download className="mr-1 h-3 w-3" />
                MD
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-7 px-2 text-xs text-muted-foreground"
                onClick={() => exportConversation("json")}
                aria-label="Export as JSON"
              >
                <Download className="mr-1 h-3 w-3" />
                JSON
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-7 px-2 text-xs text-muted-foreground"
                onClick={clear}
                aria-label="Clear conversation"
              >
                <Eraser className="mr-1 h-3 w-3" />
                Clear
              </Button>
            </div>
          )}
        </DialogHeader>

        <ScrollArea className="max-h-[60vh]">
          <div className="space-y-3 p-5">
            {history.length === 0 && (
              <div className="rounded-md border border-dashed bg-muted/30 px-4 py-6 text-center text-sm text-muted-foreground">
                Try: <em>"List every node and summarize this document."</em>
              </div>
            )}
            {history.map((t, i) => (
              <div
                key={i}
                className={
                  "rounded-md px-3 py-2 text-sm " +
                  (t.role === "user"
                    ? "bg-primary/10 text-foreground"
                    : t.role === "error"
                      ? "border border-destructive/40 bg-destructive/10 text-destructive"
                      : "border bg-muted/40 text-foreground")
                }
              >
                <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  {t.role === "user"
                    ? "You"
                    : t.role === "error"
                      ? "Error"
                      : "AIDoc AI"}
                </div>
                <div className="whitespace-pre-wrap break-words font-sans">
                  {t.role === "assistant"
                    ? renderWithLinks(t.content, nodeIds, onJumpToNode)
                    : t.content}
                </div>
              </div>
            ))}
            {pending && (
              <div className="flex items-center gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                Agent is running…
              </div>
            )}
          </div>
        </ScrollArea>

        <form
          className="relative flex items-end gap-2 border-t bg-muted/30 px-5 py-3"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <textarea
            ref={textareaRef}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (suggestion) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  moveSuggestion(1);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  moveSuggestion(-1);
                  return;
                }
                if (
                  e.key === "Enter" &&
                  !e.metaKey &&
                  !e.ctrlKey &&
                  !e.shiftKey
                ) {
                  e.preventDefault();
                  applySuggestion(suggestion.items[suggestion.index]);
                  return;
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  setSuggestion(null);
                  return;
                }
              }
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                void send();
              }
            }}
            placeholder={
              settings.openaiApiKey
                ? "Ask anything about this document…"
                : "Set OPENAI_API_KEY in Settings first."
            }
            disabled={pending}
            rows={3}
            className="min-h-[60px] flex-1 resize-none rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
          />
          {suggestion && suggestion.items.length > 0 && (
            <div className="absolute bottom-full left-0 right-12 mb-1 max-h-48 overflow-y-auto rounded-md border bg-popover p-1 text-popover-foreground shadow-md">
              {suggestion.items.map((id, i) => (
                <button
                  key={id}
                  type="button"
                  onMouseDown={(e) => {
                    // mousedown (not click) so the textarea doesn't lose focus
                    e.preventDefault();
                    applySuggestion(id);
                  }}
                  onMouseEnter={() =>
                    setSuggestion({ ...suggestion, index: i })
                  }
                  className={cn(
                    "flex w-full items-center justify-between gap-2 rounded-sm px-2 py-1 text-left text-sm",
                    i === suggestion.index && "bg-accent text-accent-foreground",
                  )}
                >
                  <span className="font-mono">@{id}</span>
                  {i === suggestion.index && (
                    <span className="text-[10px] text-muted-foreground">↵</span>
                  )}
                </button>
              ))}
              <div className="mt-1 border-t px-2 pt-1 text-[10px] text-muted-foreground">
                ↑↓ navigate · ↵ insert · esc dismiss
              </div>
            </div>
          )}
          {pending ? (
            <Button type="button" variant="destructive" onClick={() => void abort()}>
              <Square className="h-3.5 w-3.5 fill-current" />
              <span className="ml-1.5">Stop</span>
            </Button>
          ) : (
            <Button type="submit" disabled={!prompt.trim()}>
              <Send className="h-4 w-4" />
              <span className="ml-1.5">Send</span>
            </Button>
          )}
        </form>
      </DialogContent>
    </Dialog>
  );
}