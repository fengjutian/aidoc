import { useRef, useState } from "react";
import { FileImage, Upload, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ImageEditorProps {
  content: string;
  onChange: (next: string) => void;
}

const MAX_BYTES = 5 * 1024 * 1024; // 5 MiB — anything bigger should go through
// real `.aidoc/assets/` storage rather than being inlined.

function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const fr = new FileReader();
    fr.onerror = () => reject(fr.error ?? new Error("read failed"));
    fr.onload = () => resolve(String(fr.result));
    fr.readAsDataURL(file);
  });
}

/**
 * Editor for `image` kind nodes. The simplest viable workflow:
 *
 *  - Paste any URL (`https://…`, `/foo.png`, etc.) → stored as-is and
 *    rendered as `<img src=…>`.
 *  - Pick a local file → base64-inlined into the node content so the image
 *    travels with the `.aidoc` package. Capped at 5 MiB so we don't blow
 *    up revision sizes.
 *
 * Real on-disk storage under `.aidoc/assets/` is left as a future
 * schema change — the current Manifest entry model doesn't have an
 * assets/ slot, so inlining is the safest MVP.
 */
export function ImageEditor({ content, onChange }: ImageEditorProps) {
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const onPick = () => fileInputRef.current?.click();

  const onFile = async (file: File | null) => {
    if (!file) return;
    setError(null);
    if (!file.type.startsWith("image/")) {
      setError(`Not an image: ${file.type || "unknown type"}`);
      return;
    }
    if (file.size > MAX_BYTES) {
      setError(
        `File is ${(file.size / 1024 / 1024).toFixed(1)} MiB; the inline limit is 5 MiB.`,
      );
      return;
    }
    try {
      const dataUrl = await readAsDataUrl(file);
      onChange(dataUrl);
    } catch (e) {
      setError(String(e));
    }
  };

  const clear = () => {
    onChange("");
    setError(null);
  };

  return (
    <div className="flex h-full min-h-[260px] flex-col gap-3 p-3">
      <div className="flex items-center gap-2">
        <input
          className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
          placeholder="Image URL (https://…) or pick a local file"
          value={content}
          onChange={(e) => onChange(e.target.value)}
        />
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={(e) => void onFile(e.target.files?.[0] ?? null)}
        />
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-9 w-9 shrink-0"
          aria-label="Pick a local image file"
          onClick={onPick}
        >
          <Upload className="h-4 w-4" />
        </Button>
        {content && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="h-9 w-9 shrink-0"
            aria-label="Clear image"
            onClick={clear}
          >
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      <div
        className={cn(
          "flex flex-1 items-center justify-center rounded-md border bg-muted/30 p-3",
          content ? "" : "border-dashed text-muted-foreground",
        )}
      >
        {content ? (
          // eslint-disable-next-line jsx-a11y/img-redundant-alt
          <img
            src={content}
            alt="node preview"
            className="max-h-[420px] max-w-full rounded object-contain"
          />
        ) : (
          <div className="flex flex-col items-center gap-2 text-sm">
            <FileImage className="h-8 w-8 text-muted-foreground" />
            <span>No image yet — paste a URL above or click upload.</span>
          </div>
        )}
      </div>
    </div>
  );
}