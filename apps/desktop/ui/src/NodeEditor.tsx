import { useEffect, useState } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";

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
}

/**
 * Per-kind editor. Diagrams get a Mermaid viewer; everything else gets a
 * Tiptap rich-text editor. Output is plain HTML, which the AIDoc Operation
 * pipeline persists as `Patch.content`.
 */
export function NodeEditor({ kind, content, onChange }: Props) {
  if (kind === "diagram") {
    return <DiagramView source={content} onChange={onChange} editable />;
  }

  return <RichEditor kind={kind} content={content} onChange={onChange} />;
}

function RichEditor({ kind, content, onChange }: Props) {
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
        <>
          {!isCode && <Toolbar editor={editor} />}
          <EditorContent editor={editor} />
        </>
      )}
    </div>
  );
}

function Toolbar({ editor }: { editor: NonNullable<ReturnType<typeof useEditor>> }) {
  const Btn = ({ onClick, active, label }: { onClick: () => void; active: boolean; label: string }) => (
    <button onClick={onClick} className={active ? "active" : ""}>
      {label}
    </button>
  );
  return (
    <div className="rich-toolbar">
      <Btn onClick={() => editor.chain().focus().toggleBold().run()} active={editor.isActive("bold")} label="B" />
      <Btn onClick={() => editor.chain().focus().toggleItalic().run()} active={editor.isActive("italic")} label="I" />
      <Btn onClick={() => editor.chain().focus().toggleCode().run()} active={editor.isActive("code")} label="</>" />
      <Btn
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        active={editor.isActive("heading", { level: 2 })}
        label="H2"
      />
      <Btn
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        active={editor.isActive("heading", { level: 3 })}
        label="H3"
      />
      <Btn
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        active={editor.isActive("bulletList")}
        label="• list"
      />
      <Btn
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
        active={editor.isActive("orderedList")}
        label="1. list"
      />
      <Btn
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        active={editor.isActive("blockquote")}
        label="❝"
      />
      <Btn
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
        active={editor.isActive("codeBlock")}
        label="{}"
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