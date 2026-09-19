import { Keyboard } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface HelpDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface Shortcut {
  keys: string;
  desc: string;
}

const SHORTCUTS: Shortcut[] = [
  { keys: "⌘ K", desc: "Open the command palette" },
  { keys: "⌘ S", desc: "Save current document" },
  { keys: "⌘ E", desc: "Export to HTML (opens in browser)" },
  { keys: "⌘ ⇧ E", desc: "Export to Markdown (.md download)" },
  { keys: "⌘ N", desc: "Create a new section node" },
  { keys: "⌘ F", desc: "Focus the sidebar search box" },
  { keys: "⌘ ⇧ V", desc: "Validate the document" },
  { keys: "⌘ /", desc: "Toggle this help dialog" },
];

const IS_MAC =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

function displayKeys(k: string): string {
  // On non-Mac platforms, swap ⌘ for Ctrl so the chip matches what users
  // actually type.
  return IS_MAC ? k : k.replace(/⌘/g, "Ctrl");
}

export function HelpDialog({ open, onOpenChange }: HelpDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="flex items-center gap-2">
            <Keyboard className="h-4 w-4 text-primary" />
            Keyboard shortcuts
          </DialogTitle>
          <DialogDescription>
            These work whenever an editable field isn't focused.
          </DialogDescription>
        </DialogHeader>
        <ul className="divide-y">
          {SHORTCUTS.map((s) => (
            <li
              key={s.keys}
              className="flex items-center justify-between px-5 py-2 text-sm"
            >
              <span>{s.desc}</span>
              <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">
                {displayKeys(s.keys)}
              </kbd>
            </li>
          ))}
        </ul>
        <div className="flex items-center justify-end border-t bg-muted/30 px-5 py-3">
          <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}