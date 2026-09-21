import { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";
import { Eye, Pencil, Play, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useSettings } from "@/hooks/useSettings";
import { useTheme } from "@/hooks/useTheme";

// Mermaid re-init happens when the resolved theme changes. Mermaid caches its
// internal state per-theme, so we have to invalidate + re-initialise on switch.
function applyMermaidTheme(theme: "default" | "dark") {
  mermaid.initialize({
    startOnLoad: false,
    theme,
    fontFamily: "ui-sans-serif, system-ui, sans-serif",
    securityLevel: "loose",
  });
}

function pickMermaidTheme(
  override: "auto" | "default" | "dark",
  resolved: "light" | "dark",
): "default" | "dark" {
  if (override === "default" || override === "dark") return override;
  return resolved === "dark" ? "dark" : "default";
}

function cleanupMermaidArtifacts(id: string) {
  document.getElementById(id)?.remove();
  document.getElementById(`d${id}`)?.remove();
  document.querySelectorAll<HTMLElement>("body > div[id^='daidoc-mermaid-']").forEach((node) => node.remove());
}

interface Props {
  source: string;
  onChange?: (next: string) => void;
  onConvertToText?: () => void;
  editable?: boolean;
}

/**
 * Renders a Mermaid diagram from the AIDoc <diagram> node's content. Editing
 * the source falls back to a textarea; on blur we re-parse and re-render.
 */
export function DiagramView({ source, onChange, onConvertToText, editable = true }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const { resolved } = useTheme();
  const { settings } = useSettings();
  const mermaidTheme = pickMermaidTheme(settings.mermaidTheme, resolved);
  const [draft, setDraft] = useState(source);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    applyMermaidTheme(mermaidTheme);
  }, [mermaidTheme]);

  useEffect(() => {
    if (editing) return;
    let cancelled = false;
    (async () => {
      const id = `aidoc-mermaid-${Math.random().toString(36).slice(2, 9)}`;
      try {
        const diagramSource = source || "graph TD\n  A[empty]";
        await mermaid.parse(diagramSource);
        const { svg } = await mermaid.render(id, diagramSource);
        cleanupMermaidArtifacts(id);
        if (!cancelled) {
          setSvg(svg);
          setError(null);
        }
      } catch (e) {
        cleanupMermaidArtifacts(id);
        if (!cancelled) {
          setSvg(null);
          setError(String(e));
        }
      }
    })();
    return () => {
      cancelled = true;
      document.querySelectorAll<HTMLElement>("body > div[id^='daidoc-mermaid-']").forEach((node) => node.remove());
    };
  }, [source, editing, mermaidTheme]);

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
      {error ? (
        <div className="m-4 flex max-w-xl flex-col gap-3 rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
          <div>
            <div className="text-sm font-medium">这段内容不是有效的 Mermaid 图表</div>
            <div className="mt-1 text-xs text-muted-foreground">内容仍然完整保留。可以恢复为普通文本，或编辑图表源码。</div>
          </div>
          <div className="flex gap-2">
            {onConvertToText && <Button size="sm" onClick={onConvertToText}>恢复为普通文本</Button>}
            <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
              <Pencil className="mr-1.5 h-3.5 w-3.5" />编辑图表源码
            </Button>
          </div>
        </div>
      ) : (
        <div
          ref={hostRef}
          className="mermaid-svg"
          dangerouslySetInnerHTML={{ __html: svg ?? "" }}
        />
      )}
      {editable && !error && (
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
