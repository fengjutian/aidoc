import { useEffect, useState } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import {
  AlertOctagon,
  AlignLeft,
  Bold,
  BookmarkCheck,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Code,
  Code2,
  FileText,
  Heading2,
  Heading3,
  Image as ImageIcon,
  Italic,
  Link2,
  List,
  ListOrdered,
  Quote,
  Table as TableIcon,
  Workflow,
  Wrench,
  BookOpen,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { DiagramView } from "./DiagramView";

type Kind =
  | "section"
  | "paragraph"
  | "heading"
  | "code"
  | "blockquote"
  | "list"
  | "list-item"
  | "table"
  | "table-row"
  | "table-cell"
  | "link"
  | "image"
  | "diagram"
  | "code-ref"
  | "requirement"
  | "decision"
  | "problem"
  | "solution"
  | "reference"
  | "details"
  | "summary"
  | "generic";

interface Props {
  kind: Kind;
  content: string;
  onChange: (html: string) => void;
  onKindChange?: (kind: Kind) => void;
}

/**
 * Per-kind editor. Diagrams get a Mermaid viewer; everything else gets a
 * Tiptap rich-text editor. Output is plain HTML, which the AIDoc Operation
 * pipeline persists as `Patch.content`.
 */
export function NodeEditor({ kind, content, onChange, onKindChange }: Props) {
  if (kind === "diagram") {
    return <DiagramView source={content} onChange={onChange} editable />;
  }

  return (
    <RichEditor
      kind={kind}
      content={content}
      onChange={onChange}
      onKindChange={onKindChange}
    />
  );
}

function RichEditor({ kind, content, onChange, onKindChange }: Props) {
  const isCode = kind === "code";
  const isQuote = kind === "blockquote";
  const isRequirement = kind === "requirement";
  const isDecision = kind === "decision";
  const isProblem = kind === "problem";
  const isSolution = kind === "solution";
  const isCodeRef = kind === "code-ref";

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        codeBlock: { HTMLAttributes: { class: "aidoc-code" } },
        heading: isCode ? false : { levels: [2, 3, 4] },
      }),
      Link.configure({ openOnClick: false }),
      Placeholder.configure({
        placeholder: `Empty ${kind} node. Start typing…`,
      }),
    ],
    content: wrapForKind(kind, content),
    onUpdate: ({ editor }) => {
      onChange(unwrapForKind(kind, editor.getHTML()));
    },
    editorProps: {
      attributes: {
        class: [
          "aidoc-rich",
          isCode ? "is-code" : "",
          isQuote ? "is-quote" : "",
          isRequirement ? "is-requirement" : "",
          isDecision ? "is-decision" : "",
          isProblem ? "is-problem" : "",
          isSolution ? "is-solution" : "",
          isCodeRef ? "is-code-ref" : "",
        ]
          .filter(Boolean)
          .join(" "),
        "data-aidoc-kind": kind,
      },
    },
  });

  // Resync when switching nodes.
  const [lastKind, setLastKind] = useState(kind);
  const [lastContent, setLastContent] = useState(content);
  useEffect(() => {
    if (!editor) return;
    if (lastKind !== kind || lastContent !== content) {
      editor.commands.setContent(wrapForKind(kind, content), false);
      setLastKind(kind);
      setLastContent(content);
    }
  }, [kind, content, editor, lastKind, lastContent]);

  return (
    <div className="node-editor">
      {editor && (
        <TooltipProvider delayDuration={250}>
          {onKindChange && (
            <KindStrip kind={kind} onChange={onKindChange} />
          )}
          {!isCode && <Toolbar editor={editor} />}
          <EditorContent editor={editor} />
        </TooltipProvider>
      )}
    </div>
  );
}

interface KindDef {
  id: Kind;
  label: string;
  Icon: LucideIcon;
  tone: string;
}

const KINDS: KindDef[] = [
  { id: "section",     label: "Section",     Icon: FileText,       tone: "" },
  { id: "heading",     label: "Heading",     Icon: Heading2,       tone: "" },
  { id: "paragraph",   label: "Paragraph",   Icon: AlignLeft,      tone: "" },
  { id: "list",        label: "List",        Icon: List,           tone: "" },
  { id: "list-item",   label: "List item",   Icon: ListOrdered,    tone: "" },
  { id: "table",       label: "Table",       Icon: TableIcon,      tone: "" },
  { id: "table-row",   label: "Table row",   Icon: TableIcon,      tone: "" },
  { id: "table-cell",  label: "Table cell",  Icon: TableIcon,      tone: "" },
  { id: "code",        label: "Code",        Icon: Code,           tone: "" },
  { id: "blockquote",  label: "Quote",       Icon: Quote,          tone: "" },
  { id: "link",        label: "Link",        Icon: Link2,          tone: "text-cyan-500" },
  { id: "image",       label: "Image",       Icon: ImageIcon,      tone: "text-fuchsia-500" },
  { id: "diagram",     label: "Diagram",     Icon: Workflow,       tone: "" },
  { id: "code-ref",    label: "Code ref",    Icon: Code2,          tone: "text-indigo-500" },
  { id: "requirement", label: "Requirement", Icon: BookmarkCheck,  tone: "text-blue-500" },
  { id: "decision",    label: "Decision",    Icon: CheckCircle2,   tone: "text-emerald-500" },
  { id: "problem",     label: "Problem",     Icon: AlertOctagon,   tone: "text-rose-500" },
  { id: "solution",    label: "Solution",    Icon: Wrench,         tone: "text-amber-500" },
  { id: "reference",   label: "Reference",   Icon: BookOpen,       tone: "" },
  { id: "details",     label: "Details",     Icon: ChevronDown,    tone: "" },
  { id: "summary",     label: "Summary",     Icon: ChevronRight,   tone: "" },
  { id: "generic",     label: "Generic",     Icon: AlignLeft,      tone: "" },
];

function KindStrip({
  kind,
  onChange,
}: {
  kind: Kind;
  onChange: (k: Kind) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1 border-b border-border bg-muted/30 px-2 py-1.5">
      <span className="mr-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Kind
      </span>
      {KINDS.map((k) => {
        const active = k.id === kind;
        return (
          <Tooltip key={k.id}>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant={active ? "secondary" : "ghost"}
                size="sm"
                className={cn("h-7 px-2 text-xs", active && "font-semibold")}
                onClick={() => onChange(k.id)}
                aria-pressed={active}
              >
                <k.Icon className={cn("mr-1 h-3.5 w-3.5", k.tone)} />
                {k.label}
              </Button>
            </TooltipTrigger>
            <TooltipContent>Switch to {k.label}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}

function Toolbar({ editor }: { editor: NonNullable<ReturnType<typeof useEditor>> }) {
  const Tip = ({
    onClick,
    active,
    label,
    Icon,
  }: {
    onClick: () => void;
    active: boolean;
    label: string;
    Icon: LucideIcon;
  }) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={onClick}
          className={cn(
            "h-8 w-8",
            active && "bg-accent text-accent-foreground",
          )}
          aria-label={label}
          aria-pressed={active}
        >
          <Icon className="h-4 w-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
  return (
    <div className="flex flex-wrap items-center gap-0.5 border-b border-border bg-muted/40 px-2 py-1">
      <Tip
        onClick={() => editor.chain().focus().toggleBold().run()}
        active={editor.isActive("bold")}
        label="Bold"
        Icon={Bold}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleItalic().run()}
        active={editor.isActive("italic")}
        label="Italic"
        Icon={Italic}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleCode().run()}
        active={editor.isActive("code")}
        label="Inline code"
        Icon={Code}
      />
      <span className="mx-1 h-5 w-px bg-border" aria-hidden />
      <Tip
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        active={editor.isActive("heading", { level: 2 })}
        label="Heading 2"
        Icon={Heading2}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        active={editor.isActive("heading", { level: 3 })}
        label="Heading 3"
        Icon={Heading3}
      />
      <span className="mx-1 h-5 w-px bg-border" aria-hidden />
      <Tip
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        active={editor.isActive("bulletList")}
        label="Bullet list"
        Icon={List}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
        active={editor.isActive("orderedList")}
        label="Ordered list"
        Icon={ListOrdered}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        active={editor.isActive("blockquote")}
        label="Blockquote"
        Icon={Quote}
      />
      <Tip
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
        active={editor.isActive("codeBlock")}
        label="Code block"
        Icon={Code2}
      />
    </div>
  );
}

/** Convert a plain-text AIDoc payload to Tiptap-friendly HTML. */
function wrapForKind(kind: Kind, content: string): string {
  if (!content) return "<p></p>";
  if (content.trim().startsWith("<")) return content;
  switch (kind) {
    case "code":
      return `<pre><code>${escapeHtml(content)}</code></pre>`;
    case "blockquote":
    case "requirement":
    case "decision":
    case "problem":
    case "solution":
      return `<blockquote><p>${escapeHtml(content)}</p></blockquote>`;
    case "code-ref":
      return `<p><strong>code-ref</strong> — ${escapeHtml(content)}</p>`;
    case "heading":
      return `<h2>${escapeHtml(content)}</h2>`;
    case "list-item":
      return `<ul><li>${escapeHtml(content)}</li></ul>`;
    case "table":
      return `<table><tbody><tr><td>${escapeHtml(content)}</td></tr></tbody></table>`;
    case "table-row":
      return `<table><tbody><tr><td>${escapeHtml(content)}</td></tr></tbody></table>`;
    case "table-cell":
      return `<table><tbody><tr><td>${escapeHtml(content)}</td></tr></tbody></table>`;
    case "link":
      // content is the URL; the label becomes the URL itself.
      return `<p><a href="${escapeHtml(content)}">${escapeHtml(content)}</a></p>`;
    case "image":
      // content is the image URL; renders as <img> in HTML export.
      return `<p><img src="${escapeHtml(content)}" alt="image"/></p>`;
    case "details":
      return `<details><summary>Details</summary><p>${escapeHtml(content)}</p></details>`;
    case "summary":
      return `<summary>${escapeHtml(content)}</summary>`;
    case "generic":
    case "paragraph":
    case "section":
    case "reference":
    case "list":
    default:
      // Paragraph / generic: each newline becomes a new <p>.
      return content
        .split(/\n+/)
        .map((l) => `<p>${escapeHtml(l)}</p>`)
        .join("");
  }
}

function unwrapForKind(kind: Kind, html: string): string {
  switch (kind) {
    case "code":
      return stripTags(html, "pre,code");
    case "blockquote":
    case "requirement":
    case "decision":
    case "problem":
    case "solution":
      return stripTags(html, "blockquote,p");
    case "code-ref":
    case "heading":
      return stripTags(html, "h2,p,h3,h4,strong");
    case "link":
    case "image": {
      // Pull the URL back out of href / src.
      const m = html.match(/(?:href|src)="([^"]+)"/);
      return m ? m[1] : stripTags(html, "p,a,img");
    }
    case "list-item":
      return stripTags(html, "ul,li");
    case "table":
    case "table-row":
    case "table-cell":
      return stripTags(html, "table,tbody,tr,td");
    case "details":
      return stripTags(html, "details,summary,p");
    case "summary":
      return stripTags(html, "summary");
    default:
      return stripTags(html, "p");
  }
}

function stripTags(html: string, _selectors: string): string {
  // Cheap strip: remove any matching wrapper + inner tags, keep text.
  // Good enough for round-tripping the simple kinds we ship.
  const out = html
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/<\/?(?:p|h[1-6]|li|ul|ol|blockquote|pre|code|strong|em|span)[^>]*>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .trim();
  return out;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}