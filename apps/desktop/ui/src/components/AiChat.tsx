import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, Eraser, Loader2, Send, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { Settings } from "@/hooks/useSettings";

interface AiChatProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  settings: Settings;
  docPath: string | null;
  nodeIds: string[];
  onJumpToNode: (id: string) => void;
}

interface ChatTurn {
  role: "user" | "assistant" | "error";
  content: string;
}

const STORAGE_KEY = "aidoc-ai-chat";

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

function loadHistory(): ChatTurn[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as ChatTurn[]) : [];
  } catch {
    return [];
  }
}

export function AiChat({
  open,
  onOpenChange,
  settings,
  docPath,
  nodeIds,
  onJumpToNode,
}: AiChatProps) {
  const [prompt, setPrompt] = useState("");
  const [history, setHistory] = useState<ChatTurn[]>(loadHistory);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (history.length === 0) {
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch {
        // ignore
      }
    } else {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(history));
      } catch {
        // ignore quota / privacy errors
      }
    }
  }, [history]);

  useEffect(() => {
    if (!open) return;
    setPrompt("");
  }, [open]);

  const clear = () => {
    setHistory([]);
    setPrompt("");
  };

  const exportMarkdown = () => {
    const stamp = new Date().toISOString().slice(0, 16).replace("T", " ");
    const lines: string[] = [];
    lines.push(`# AIDoc AI Chat — ${stamp}`);
    if (docPath) lines.push(`\n_Document: \`${docPath}\`_\n`);
    for (const t of history) {
      if (t.role === "user") lines.push(`\n## You\n\n${t.content}\n`);
      else if (t.role === "assistant") lines.push(`\n## AIDoc AI\n\n${t.content}\n`);
      else lines.push(`\n## ⚠ Error\n\n\`\`\`\n${t.content}\n\`\`\`\n`);
    }
    const blob = new Blob([lines.join("")], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `aidoc-chat-${new Date().toISOString().slice(0, 10)}.md`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const send = async () => {
    const text = prompt.trim();
    if (!text || pending) return;
    const prior = history;
    const next = [...prior, { role: "user" as const, content: text }];
    setHistory(next);
    setPrompt("");
    setPending(true);
    try {
      const out = await invoke<string>("ai_chat", {
        prompt: text,
        apiKey: settings.openaiApiKey,
        baseUrl: settings.openaiBaseUrl,
        model: settings.openaiModel,
        docPath,
        history: prior,
      });
      setHistory((h) => [
        ...h,
        { role: "assistant", content: out || "(empty response)" },
      ]);
    } catch (e) {
      setHistory((h) => [
        ...h,
        { role: "error", content: String(e) },
      ]);
    } finally {
      setPending(false);
    }
  };

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
                onClick={exportMarkdown}
                aria-label="Export as Markdown"
              >
                <Download className="mr-1 h-3 w-3" />
                Export
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
          className="flex items-end gap-2 border-t bg-muted/30 px-5 py-3"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
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
          <Button type="submit" disabled={pending || !prompt.trim()}>
            {pending ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Send className="h-4 w-4" />
            )}
            <span className="ml-1.5">Send</span>
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}