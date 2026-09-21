/**
 * Plain-text ↔ Tiptap-friendly HTML round-tripping for each node kind.
 * Extracted from `NodeEditor.tsx` so it can be unit-tested without dragging
 * React / Tiptap into the test runner. JSX-free on purpose.
 */

export type Kind =
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

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function stripTags(html: string): string {
  // Preserve visual block boundaries before dropping markup. Without this,
  // `<p>one</p><p>two</p>` became `onetwo` after the first edit/save cycle.
  return html
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/<\/(?:p|h[1-6]|li|blockquote|pre|tr|details|summary)>/g, "\n")
    .replace(/<\/?(?:p|h[1-6]|li|ul|ol|blockquote|pre|code|strong|em|span|details|summary|table|tbody|tr|td|img|a)[^>]*>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .trim();
}

export function wrapForKind(kind: Kind, content: string): string {
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
      return content
        .split(/\n+/)
        .map((line, index) => index === 0 ? `<h2>${escapeHtml(line)}</h2>` : `<p>${escapeHtml(line)}</p>`)
        .join("");
    case "list-item":
      return `<ul><li>${escapeHtml(content)}</li></ul>`;
    case "table":
    case "table-row":
    case "table-cell":
      return `<table><tbody><tr><td>${escapeHtml(content)}</td></tr></tbody></table>`;
    case "link":
      return `<p><a href="${escapeHtml(content)}">${escapeHtml(content)}</a></p>`;
    case "image":
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
      return content
        .split(/\n+/)
        .map((l) => `<p>${escapeHtml(l)}</p>`)
        .join("");
  }
}

export function unwrapForKind(kind: Kind, html: string): string {
  switch (kind) {
    case "code":
      return stripTags(html);
    case "blockquote":
    case "requirement":
    case "decision":
    case "problem":
    case "solution":
      return stripTags(html);
    case "code-ref":
    case "heading":
      return stripTags(html);
    case "link":
    case "image": {
      const m = html.match(/(?:href|src)="([^"]+)"/);
      return m ? m[1] : stripTags(html);
    }
    case "list-item":
      return stripTags(html);
    case "table":
    case "table-row":
    case "table-cell":
      return stripTags(html);
    case "details":
      return stripTags(html);
    case "summary":
      return stripTags(html);
    default:
      return stripTags(html);
  }
}
