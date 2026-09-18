import { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";

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
        />
        <div className="actions">
          <button
            onClick={() => {
              onChange?.(draft);
              setEditing(false);
            }}
          >
            render
          </button>
          <button
            onClick={() => {
              setDraft(source);
              setEditing(false);
            }}
          >
            cancel
          </button>
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
      {error && <div className="error">{error}</div>}
      {editable && (
        <div className="actions">
          <button onClick={() => setEditing(true)}>edit source</button>
        </div>
      )}
    </div>
  );
}