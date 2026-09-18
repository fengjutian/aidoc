import { Github, Info, KeyRound, RotateCcw, Save, Type, Workflow } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import type { Settings } from "@/hooks/useSettings";

interface SettingsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  settings: Settings;
  onUpdate: (patch: Partial<Settings>) => void;
}

interface RowProps {
  icon: LucideIcon;
  title: string;
  description: string;
  children: React.ReactNode;
}

function Row({ icon: Icon, title, description, children }: RowProps) {
  return (
    <div className="flex items-start justify-between gap-4 py-3">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md border bg-muted/40 text-muted-foreground">
          <Icon className="h-3.5 w-3.5" />
        </span>
        <div>
          <div className="text-sm font-medium">{title}</div>
          <div className="text-xs text-muted-foreground">{description}</div>
        </div>
      </div>
      <div className="pt-1">{children}</div>
    </div>
  );
}

function FontSizePicker({
  value,
  onChange,
}: {
  value: Settings["fontSize"];
  onChange: (v: Settings["fontSize"]) => void;
}) {
  const options: { id: Settings["fontSize"]; label: string }[] = [
    { id: "sm", label: "S" },
    { id: "md", label: "M" },
    { id: "lg", label: "L" },
  ];
  return (
    <div className="inline-flex rounded-md border bg-muted/30 p-0.5">
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          onClick={() => onChange(o.id)}
          className={
            "h-7 w-8 rounded text-xs font-medium transition-colors " +
            (value === o.id
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground")
          }
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

function MermaidThemePicker({
  value,
  onChange,
}: {
  value: Settings["mermaidTheme"];
  onChange: (v: Settings["mermaidTheme"]) => void;
}) {
  const options: { id: Settings["mermaidTheme"]; label: string }[] = [
    { id: "auto", label: "Auto" },
    { id: "default", label: "Light" },
    { id: "dark", label: "Dark" },
  ];
  return (
    <div className="inline-flex rounded-md border bg-muted/30 p-0.5">
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          onClick={() => onChange(o.id)}
          className={
            "h-7 px-2 rounded text-xs font-medium transition-colors " +
            (value === o.id
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground")
          }
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function SettingsPanel({
  open,
  onOpenChange,
  settings,
  onUpdate,
}: SettingsPanelProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg gap-0 p-0">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Preferences are saved locally to this browser profile.
          </DialogDescription>
        </DialogHeader>

        <div className="px-5">
          <Row
            icon={Save}
            title="Autosave"
            description="Persist every edit immediately to the .aidoc package."
          >
            <Switch
              checked={settings.autosave}
              onCheckedChange={(v) => onUpdate({ autosave: v })}
            />
          </Row>

          <Separator />

          <Row
            icon={Type}
            title="Editor font size"
            description="Affects the Tiptap rich-text body."
          >
            <FontSizePicker
              value={settings.fontSize}
              onChange={(v) => onUpdate({ fontSize: v })}
            />
          </Row>

          <Separator />

          <Row
            icon={Workflow}
            title="Mermaid theme"
            description='Auto follows the app theme; pick Light or Dark to override.'
          >
            <MermaidThemePicker
              value={settings.mermaidTheme}
              onChange={(v) => onUpdate({ mermaidTheme: v })}
            />
          </Row>

          <Separator />

          <div className="py-3">
            <div className="flex items-center gap-3">
              <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md border bg-muted/40 text-muted-foreground">
                <KeyRound className="h-3.5 w-3.5" />
              </span>
              <div>
                <div className="text-sm font-medium">AI / OpenAI</div>
                <div className="text-xs text-muted-foreground">
                  Used by the Ask AI panel; stored locally in this browser profile.
                </div>
              </div>
            </div>
            <div className="mt-3 grid gap-2">
              <input
                type="password"
                placeholder="API key (sk-…)"
                value={settings.openaiApiKey}
                onChange={(e) => onUpdate({ openaiApiKey: e.target.value })}
                className="h-9 rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
              />
              <input
                placeholder="Base URL (default: https://api.openai.com/v1)"
                value={settings.openaiBaseUrl}
                onChange={(e) => onUpdate({ openaiBaseUrl: e.target.value })}
                className="h-9 rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
              />
              <input
                placeholder="Model (default: gpt-4o-mini)"
                value={settings.openaiModel}
                onChange={(e) => onUpdate({ openaiModel: e.target.value })}
                className="h-9 rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-1 focus-visible:ring-ring"
              />
            </div>
          </div>
        </div>

        <Separator />

        <div className="px-5 py-4">
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            <Info className="h-3.5 w-3.5" />
            About
          </div>
          <div className="rounded-md border bg-muted/30 p-3 text-xs text-muted-foreground">
            <div className="flex items-center justify-between">
              <span>AIDoc Desktop</span>
              <code className="font-mono">v0.1.0</code>
            </div>
            <div className="mt-1 flex items-center justify-between">
              <span>Engine</span>
              <code className="font-mono">Rust · aidoc-* crates</code>
            </div>
            <div className="mt-1 flex items-center justify-between">
              <span>License</span>
              <code className="font-mono">MIT OR Apache-2.0</code>
            </div>
            <a
              className="mt-2 inline-flex items-center gap-1 text-primary hover:underline"
              href="https://github.com"
              target="_blank"
              rel="noreferrer"
            >
              <Github className="h-3 w-3" />
              Project repo
            </a>
          </div>
        </div>

        <div className="flex items-center justify-between border-t bg-muted/30 px-5 py-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={() =>
              onUpdate({ autosave: false, fontSize: "md", mermaidTheme: "auto" })
            }
          >
            <RotateCcw className="mr-1.5 h-3.5 w-3.5" />
            Reset to defaults
          </Button>
          <Button size="sm" onClick={() => onOpenChange(false)}>
            Done
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}