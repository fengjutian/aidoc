import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Code2, Copy, ExternalLink, FileCode, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";

interface Props {
  content: string;
  attributes: Record<string, string>;
  onChange: (next: string) => void;
  onAttributesChange: (attributes: Record<string, string>) => void;
}

export function CodeRefEditor({ content, attributes, onChange, onAttributesChange }: Props) {
  const [source, setSource] = useState(attributes.source ?? attributes.file ?? "");
  const [line, setLine] = useState(attributes.line ?? "");
  const [copied, setCopied] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSource(attributes.source ?? attributes.file ?? "");
    setLine(attributes.line ?? "");
  }, [attributes]);

  const reference = source && line ? `${source}:${line}` : source || line;
  const saveAttributes = () => {
    if (source === (attributes.source ?? attributes.file ?? "") && line === (attributes.line ?? "")) return;
    onAttributesChange({ ...attributes, source, line });
  };
  const copy = async (label: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      window.setTimeout(() => setCopied(null), 1500);
    } catch { /* clipboard can be unavailable */ }
  };
  const openSource = async () => {
    setOpening(true);
    setError(null);
    try {
      const parsed = line.trim() ? Number(line) : undefined;
      if (parsed !== undefined && (!Number.isInteger(parsed) || parsed < 1)) throw new Error("Line must be a positive integer.");
      await invoke<string>("open_code_ref", { source, line: parsed });
    } catch (e) {
      setError(String(e));
    } finally {
      setOpening(false);
    }
  };

  return <div className="flex h-full min-h-[260px] flex-col gap-4 p-3">
    <div className="flex items-center gap-2 text-sm"><Code2 className="h-4 w-4 text-indigo-500" /><span className="font-medium">Code reference</span><span className="text-xs text-muted-foreground">{source || line ? "—" : "no source set"}</span></div>
    <div className="grid grid-cols-[100px_1fr_auto] items-center gap-2 text-sm">
      <label className="text-xs uppercase tracking-wider text-muted-foreground">source</label>
      <input className="h-9 w-full rounded-md border border-input bg-background px-3 font-mono text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring" placeholder="path/to/file.py" value={source} onChange={(e) => setSource(e.target.value)} onBlur={saveAttributes} />
      <Button type="button" variant="outline" size="icon" className="h-9 w-9" aria-label="Copy source path" disabled={!source} onClick={() => void copy("source", source)}><Copy className="h-3.5 w-3.5" /></Button>
      <label className="text-xs uppercase tracking-wider text-muted-foreground">line</label>
      <input className="h-9 w-full rounded-md border border-input bg-background px-3 font-mono text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring" placeholder="42" inputMode="numeric" value={line} onChange={(e) => setLine(e.target.value)} onBlur={saveAttributes} />
      <Button type="button" variant="outline" size="icon" className="h-9 w-9" aria-label="Copy line number" disabled={!line} onClick={() => void copy("line", line)}><Copy className="h-3.5 w-3.5" /></Button>
    </div>
    {reference && <div className="flex gap-2">
      <Button type="button" variant="secondary" className="min-w-0 flex-1 justify-start font-mono" onClick={() => void copy("ref", reference)}><FileCode className="mr-2 h-4 w-4" /><span className="truncate">{reference}</span>{copied === "ref" ? <span className="ml-auto text-[10px] text-emerald-500">copied</span> : <Copy className="ml-auto h-3.5 w-3.5" />}</Button>
      <Button type="button" onClick={() => void openSource()} disabled={opening || !source}>{opening ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <ExternalLink className="mr-2 h-4 w-4" />}Open</Button>
    </div>}
    {error && <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</div>}
    <div className="flex-1 rounded-md border bg-muted/30 p-3"><label className="text-xs uppercase tracking-wider text-muted-foreground">Note</label><textarea className="mt-2 h-full min-h-[140px] w-full resize-none bg-transparent text-sm outline-none" placeholder="Optional annotation that travels with the reference…" value={content} onChange={(e) => onChange(e.target.value)} /></div>
    <div className="flex items-center gap-2 text-xs text-muted-foreground"><ExternalLink className="h-3.5 w-3.5" />Relative paths resolve beside the .aidoc file or from AIDOC_SOURCE_ROOT. VS Code opens at the requested line when available.</div>
  </div>;
}
