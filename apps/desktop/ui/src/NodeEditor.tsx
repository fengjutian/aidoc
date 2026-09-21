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
  MoreHorizontal,
  Quote,
  Table as TableIcon,
  Workflow,
  Wrench,
  BookOpen,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { DiagramView } from "./DiagramView";
import { ImageEditor } from "./ImageEditor";
import { CodeRefEditor } from "./CodeRefEditor";
import { wrapForKind, unwrapForKind } from "./editorSerialize";

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
  attributes?: Record<string, string>;
  onChange: (html: string) => void;
  onAttributesChange?: (attributes: Record<string, string>) => void;
  onKindChange?: (kind: Kind) => void;
}

/**
 * Per-kind editor. Diagrams get a Mermaid viewer; everything else gets a
 * Tiptap rich-text editor. Output is plain HTML, which the AIDoc Operation
 * pipeline persists as `Patch.content`.
 */
export function NodeEditor({ kind, content, onChange, onKindChange, attributes, onAttributesChange }: Props) {
  if (kind === "diagram") {
    return <DiagramView source={content} onChange={onChange} onConvertToText={() => onKindChange?.("paragraph")} editable />;
  }
  if (kind === "image") {
    return <ImageEditor content={content} onChange={onChange} />;
  }
  if (kind === "code-ref") {
    return (
      <CodeRefEditor
        content={content}
        attributes={attributes ?? {}}
        onChange={onChange}
        onAttributesChange={onAttributesChange ?? (() => {})}
      />
    );
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
            <KindPicker kind={kind} onChange={onKindChange} />
          )}
          {!isCode && <EditorToolbar editor={editor} />}
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
  group: "Block" | "Lists" | "Tables" | "Media" | "Semantic";
}

const KINDS: KindDef[] = [
  { id: "section",     label: "Section",     Icon: FileText,       tone: "",              group: "Block" },
  { id: "heading",     label: "Heading",     Icon: Heading2,       tone: "",              group: "Block" },
  { id: "paragraph",   label: "Paragraph",   Icon: AlignLeft,      tone: "",              group: "Block" },
  { id: "blockquote",  label: "Quote",       Icon: Quote,          tone: "",              group: "Block" },
  { id: "code",        label: "Code",        Icon: Code,           tone: "",              group: "Block" },
  { id: "list",        label: "List",        Icon: List,           tone: "",              group: "Lists" },
  { id: "list-item",   label: "List item",   Icon: ListOrdered,    tone: "",              group: "Lists" },
  { id: "table",       label: "Table",       Icon: TableIcon,      tone: "",              group: "Tables" },
  { id: "table-row",   label: "Table row",   Icon: TableIcon,      tone: "",              group: "Tables" },
  { id: "table-cell",  label: "Table cell",  Icon: TableIcon,      tone: "",              group: "Tables" },
  { id: "link",        label: "Link",        Icon: Link2,          tone: "text-cyan-500",  group: "Media" },
  { id: "image",       label: "Image",       Icon: ImageIcon,      tone: "text-fuchsia-500", group: "Media" },
  { id: "diagram",     label: "Diagram",     Icon: Workflow,       tone: "",              group: "Media" },
  { id: "code-ref",    label: "Code ref",    Icon: Code2,          tone: "text-indigo-500", group: "Media" },
  { id: "requirement", label: "Requirement", Icon: BookmarkCheck,  tone: "text-blue-500",  group: "Semantic" },
  { id: "decision",    label: "Decision",    Icon: CheckCircle2,   tone: "text-emerald-500", group: "Semantic" },
  { id: "problem",     label: "Problem",     Icon: AlertOctagon,   tone: "text-rose-500",  group: "Semantic" },
  { id: "solution",    label: "Solution",    Icon: Wrench,         tone: "text-amber-500", group: "Semantic" },
  { id: "reference",   label: "Reference",   Icon: BookOpen,       tone: "",              group: "Semantic" },
  { id: "details",     label: "Details",     Icon: ChevronDown,    tone: "",              group: "Semantic" },
  { id: "summary",     label: "Summary",     Icon: ChevronRight,   tone: "",              group: "Semantic" },
  { id: "generic",     label: "Generic",     Icon: AlignLeft,      tone: "",              group: "Semantic" },
];

const KIND_GROUPS: KindDef["group"][] = ["Block", "Lists", "Tables", "Media", "Semantic"];

/**
 * Common kinds shown as one-click chips in the picker — covers ~90% of edits.
 * Everything else is one submenu away.
 */
const FAVORITE_KINDS: Kind[] = [
  "section",
  "heading",
  "paragraph",
  "blockquote",
  "code",
  "list",
];

/**
 * Compact kind picker. Common picks are one click; the rest are grouped into
 * hover-to-open submenus so the main menu never gets longer than ~6 rows.
 */
function KindPicker({
  kind,
  onChange,
}: {
  kind: Kind;
  onChange: (k: Kind) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1 border-b border-border bg-muted/30 px-3 py-2">
      <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Block
      </span>
      {FAVORITE_KINDS.map((id) => {
        const item = KINDS.find((candidate) => candidate.id === id)!;
        return (
          <Button
            key={item.id}
            type="button"
            variant={item.id === kind ? "secondary" : "ghost"}
            size="sm"
            className={cn("h-7 gap-1.5 px-2", item.id === kind && "ring-1 ring-border")}
            onClick={() => onChange(item.id)}
            aria-pressed={item.id === kind}
          >
            <item.Icon className={cn("h-3.5 w-3.5", item.tone)} />
            {item.label}
          </Button>
        );
      })}
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 gap-1.5 px-2 text-xs font-semibold"
                aria-label="More block types"
              >
                <MoreHorizontal className="h-3.5 w-3.5" />
                More
                <ChevronDown className="h-3 w-3 opacity-60" />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>Switch node kind</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="start" className="w-48">
          {KIND_GROUPS.map((group) => {
            const items = KINDS.filter((k) => k.group === group && !FAVORITE_KINDS.includes(k.id));
            if (items.length === 0) return null;
            return (
              <DropdownMenuSub key={group}>
                <DropdownMenuSubTrigger>
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    {group}
                  </span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {items.length}
                  </span>
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent sideOffset={4} className="w-44">
                  {items.map((k) => (
                    <DropdownMenuItem
                      key={k.id}
                      onSelect={() => {
                        const special = ["diagram", "image", "code-ref"].includes(k.id);
                        if (!special || window.confirm(`将当前节点转换为 ${k.label}？现有内容会按该类型解析。`)) {
                          onChange(k.id);
                        }
                      }}
                      className={cn(
                        k.id === kind && "font-semibold text-primary",
                      )}
                    >
                      <k.Icon
                        className={cn(
                          "h-4 w-4",
                          k.tone || "text-muted-foreground",
                        )}
                      />
                      <span className="flex-1">{k.label}</span>
                      {k.id === kind && (
                        <CheckCircle2 className="h-3.5 w-3.5 text-primary" />
                      )}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            );
          })}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

/**
 * Floating inline toolbar — appears above the current text selection instead
 * of pinning itself above the editor like the previous static toolbar.
 */
function EditorToolbar({
  editor,
}: {
  editor: NonNullable<ReturnType<typeof useEditor>>;
}) {
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
            "h-7 w-7",
            active && "bg-accent text-accent-foreground",
          )}
          aria-label={label}
          aria-pressed={active}
        >
          <Icon className="h-3.5 w-3.5" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
  return (
    <div className="flex flex-wrap items-center gap-0.5 border-b border-border bg-card px-3 py-1.5">
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
      <span className="mx-0.5 h-4 w-px bg-border" aria-hidden />
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
      <span className="mx-0.5 h-4 w-px bg-border" aria-hidden />
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


// wrapForKind / unwrapForKind live in ./editorSerialize (JSX-free) so
// they can be unit-tested with node --test without dragging React / Tiptap
// into the test runner. Re-exported here so existing imports keep working.
export { wrapForKind, unwrapForKind } from "./editorSerialize";
