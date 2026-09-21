import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FileImage, Loader2, Upload, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ImageEditorProps {
  content: string;
  onChange: (next: string) => void;
}

interface ImportedAsset {
  path: string;
  data_url: string;
}

export function ImageEditor({ content, onChange }: ImageEditorProps) {
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState(content.startsWith("assets/") ? "" : content);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (!content.startsWith("assets/")) {
      setPreview(content);
      return;
    }
    setPreview("");
    void invoke<string>("resolve_image_asset", { path: content })
      .then((url) => { if (!cancelled) setPreview(url); })
      .catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, [content]);

  const onPick = async () => {
    setError(null);
    const sourcePath = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"] }],
    });
    if (typeof sourcePath !== "string") return;
    setBusy(true);
    try {
      const asset = await invoke<ImportedAsset>("import_image_asset", { sourcePath });
      setPreview(asset.data_url);
      onChange(asset.path);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const updateUrl = (value: string) => {
    setError(null);
    setPreview(value);
    onChange(value);
  };

  return (
    <div className="flex h-full min-h-[260px] flex-col gap-3 p-3">
      <div className="flex items-center gap-2">
        <input
          className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
          placeholder="Image URL or packaged assets/ path"
          value={content}
          onChange={(e) => updateUrl(e.target.value)}
        />
        <Button type="button" variant="outline" size="icon" className="h-9 w-9 shrink-0" aria-label="Import image into document package" onClick={() => void onPick()} disabled={busy}>
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
        </Button>
        {content && (
          <Button type="button" variant="ghost" size="icon" className="h-9 w-9 shrink-0" aria-label="Clear image" onClick={() => updateUrl("")}>
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>
      {content.startsWith("assets/") && <div className="text-xs text-muted-foreground">Stored inside this .aidoc package as <code>{content}</code>.</div>}
      {error && <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</div>}
      <div className={cn("flex flex-1 items-center justify-center rounded-md border bg-muted/30 p-3", preview ? "" : "border-dashed text-muted-foreground")}>
        {preview ? <img src={preview} alt="node preview" onError={() => setError("Unable to load this image. Check the URL or import a local file.")} onLoad={() => setError(null)} className="max-h-[420px] max-w-full rounded object-contain" /> : <div className="flex flex-col items-center gap-2 text-sm"><FileImage className="h-8 w-8" /><span>No image yet — paste a URL or import a local image.</span></div>}
      </div>
    </div>
  );
}
