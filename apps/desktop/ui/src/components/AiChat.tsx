import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Send, Sparkles } from "lucide-react";

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
}

interface ChatTurn {
  role: "user" | "assistant" | "error";
  content: string;
}

export function AiChat({ open, onOpenChange, settings, docPath }: AiChatProps) {
  const [prompt, setPrompt] = useState("");
  const [history, setHistory] = useState<ChatTurn[]>([]);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (!open) return;
    setPrompt("");
  }, [open]);

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
        <DialogHeader className="border-b px-5 py-4">
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
                <pre className="whitespace-pre-wrap break-words font-sans">
                  {t.content}
                </pre>
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