import { useState } from "react";
import { Code2, Copy, ExternalLink, FileCode } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface CodeRefEditorProps {
  content: string;
  attributes: Record<string, string>;
  onChange: (next: string) => void;
  onAttributesChange: () => void;
}

/**
 * Editor for `code-ref` kind nodes. AIDoc stores source + line in the
 * node's `attributes` map; we surface them here with copy buttons so a
 * reader can paste the reference into their editor of choice.
 *
 * Real "open in editor" jumps require a shell-plugin that isn't bundled
 * yet — clicking Copy + `git grep` is the pragmatic workaround.
 */
export function CodeRefEditor({
  content,
  attributes,
  onChange,
  onAttributesChange,
}: CodeRefEditorProps) {
  const [copied, setCopied] = useState<string | null>(null);
  const source = attributes.source ?? "";
  const line = attributes.line ?? "";

  const copy = async (label: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      window.setTimeout(() => setCopied(null), 1500);
    } catch {
      // ignore — clipboard not available in some sandboxes
    }
  };

  const reference = source && line ? `${source}:${line}` : source || line;

  return (
    <div className="flex h-full min-h-[260px] flex-col gap-4 p-3">
      <div className="flex items-center gap-2 text-sm">
        <Code2 className="h-4 w-4 text-indigo-500" />
        <span className="font-medium">Code reference</span>
        <span className="text-xs text-muted-foreground">
          {source || line ? "—" : "no source set"}
        </span>
      </div>

      <div className="grid grid-cols-[100px_1fr_auto] items-center gap-2 text-sm">
        <label className="text-xs uppercase tracking-wider text-muted-foreground">
          source
        </label>
        <input
          className="h-9 w-full rounded-md border border-input bg-background px-3 font-mono text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
          placeholder="path/to/file.py"
          value={source}
          onChange={(e) => {
            attributes.source = e.target.value;
            onAttributesChange();
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-9 w-9 shrink-0"
          aria-label="Copy source path"
          disabled={!source}
          onClick={() => void copy("source", source)}
        >
          {copied === "source" ? (
            <span className="text-[10px] font-semibold text-emerald-500">✓</span>
          ) : (
            <Copy className="h-3.5 w-3.5" />
          )}
        </Button>

        <label className="text-xs uppercase tracking-wider text-muted-foreground">
          line
        </label>
        <input
          className="h-9 w-full rounded-md border border-input bg-background px-3 font-mono text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
          placeholder="42"
          value={line}
          onChange={(e) => {
            attributes.line = e.target.value;
            onAttributesChange();
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-9 w-9 shrink-0"
          aria-label="Copy line number"
          disabled={!line}
          onClick={() => void copy("line", line)}
        >
          {copied === "line" ? (
            <span className="text-[10px] font-semibold text-emerald-500">✓</span>
          ) : (
            <Copy className="h-3.5 w-3.5" />
          )}
        </Button>
      </div>

      {reference && (
        <Button
          type="button"
          variant="secondary"
          className="justify-start font-mono"
          onClick={() => void copy("ref", reference)}
        >
          <FileCode className="mr-2 h-4 w-4" />
          {reference}
          {copied === "ref" ? (
            <span className="ml-auto text-[10px] uppercase tracking-wider text-emerald-500">
              copied
            </span>
          ) : (
            <Copy className="ml-auto h-3.5 w-3.5" />
          )}
        </Button>
      )}

      <div className="flex-1 rounded-md border bg-muted/30 p-3">
        <label className="text-xs uppercase tracking-wider text-muted-foreground">
          Note
        </label>
        <textarea
          className="mt-2 h-full min-h-[140px] w-full resize-none bg-transparent text-sm outline-none"
          placeholder="Optional annotation that will travel with the reference…"
          value={content}
          onChange={(e) => onChange(e.target.value)}
        />
      </div>

      <div className={cn("flex items-center gap-2 text-xs text-muted-foreground")}>
        <ExternalLink className="h-3.5 w-3.5" />
        Set source + line via Attributes (right-click the node) or the inputs
        above. Use Copy to paste the reference into your editor.
      </div>
    </div>
  );
}