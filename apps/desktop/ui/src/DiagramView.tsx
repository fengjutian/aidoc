import { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";
import { Eye, Pencil, Play, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

// Initialise once.
mermaid.initialize({
  startOnLoad: false,
  theme: "default",
  fontFamily: "ui-sans-serif, system-ui, sans-serif",
  securityLevel: "loose",
});

interface Props {
  source: string;
  onChange?: (next: string) => void;
  editable?: boolean;
}

/**
 * Renders a Mermaid diagram from the AIDoc <diagram> node's content. Editing
 * the source falls back to a textarea; on blur we re-parse and re-render.
 */
export function DiagramView({ source, onChange, editable = true }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [draft, setDraft] = useState(source);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    if (editing) return;
    let cancelled = false;
    (async () => {
      try {
        const id = `aidoc-mermaid-${Math.random().toString(36).slice(2, 9)}`;
        const { svg } = await mermaid.render(id, source || "graph TD\n  A[empty]");
        if (!cancelled) {
          setSvg(svg);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setSvg(null);
          setError(String(e));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [source, editing]);

  if (editing) {
    return (
      <div className="diagram-edit">
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          spellCheck={false}
          className="h-full min-h-[280px] w-full resize-none rounded-md border border-input bg-background px-3 py-2 font-mono text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
        />
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            onClick={() => {
              onChange?.(draft);
              setEditing(false);
            }}
          >
            <Play className="mr-1.5 h-3.5 w-3.5" />
            Render
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setDraft(source);
              setEditing(false);
            }}
          >
            <X className="mr-1.5 h-3.5 w-3.5" />
            Cancel
          </Button>
          <Separator orientation="vertical" className="mx-1 h-5" />
          <span className="text-xs text-muted-foreground">
            Mermaid source · <code className="font-mono">{draft.length}</code> chars
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="diagram-view">
      <div
        ref={hostRef}
        className="mermaid-svg"
        dangerouslySetInnerHTML={{ __html: svg ?? "" }}
      />
      {error && (
        <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}
      {editable && (
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
            <Pencil className="mr-1.5 h-3.5 w-3.5" />
            Edit source
          </Button>
          <span className="text-xs text-muted-foreground">
            <Eye className="mr-1 inline h-3 w-3" />
            Live preview
          </span>
        </div>
      )}
    </div>
  );
}