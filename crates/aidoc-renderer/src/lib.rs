//! AIDoc → HTML rendering.
//!
//! Inputs: a flat list of `Node`s + a single root `NodeId`.
//! Output: a complete HTML document string.
//!
//! Layout: we treat all nodes with `parent == None` (other than the root) as
//! orphans and skip them with a warning. Children are emitted in `position`
//! order.

use std::collections::HashMap;
use std::fmt::Write as _;

use aidoc_model::{Document, Node, NodeKind, Relation};

/// Render an AIDoc document to HTML.
///
/// `branch` is the label of the branch the rendered revision belongs to
/// (e.g. "ai-draft"). When `Some`, a small `<aside class="aidoc-branch">`
/// banner is inserted above the document body. When `None`, no banner is
/// shown — the document is treated as on main.
pub fn render_html(
    doc: &Document,
    nodes: &[Node],
    relations: &[Relation],
    branch: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    let _ = write!(
        out,
        "  <meta charset=\"utf-8\">\n  <title>{}</title>\n",
        escape(&doc.title)
    );
    out.push_str(
        "  <style>\
         :root{color-scheme:light dark}\
         body{font-family:ui-sans-serif,system-ui,sans-serif;margin:1.5rem auto;max-width:48rem;line-height:1.55;padding:0 1rem}\
         .aidoc-toc{background:#f5f5f5;border:1px solid #ddd;border-radius:6px;padding:.5em 1em;margin:0 0 1.5em 0}\
         .aidoc-toc summary{font-weight:600;cursor:pointer;padding:.25em 0}\
         .aidoc-toc ul{margin:.25em 0;padding-left:1.25em;list-style:none}\
         .aidoc-toc li{margin:.15em 0}\
         .aidoc-toc a{color:#444;text-decoration:none}\
         .aidoc-toc a:hover{text-decoration:underline;color:#2563eb}\
         .aidoc-diagram{font-family:monospace;background:#f8f8f8;padding:.5em;border-radius:4px;overflow:auto}\
         .aidoc-branch{background:#eef;border-left:4px solid #88c;\
                       padding:.5em 1em;margin:0 0 1em 0;font-family:monospace}\
         .aidoc-branch::before{content:\"branch: \"}\
         .aidoc-relations{font-size:.85em;color:#666;border-left:2px solid #ddd;padding:.25em .75em;margin:.4em 0 1em}\
         .aidoc-relations ul{display:flex;flex-wrap:wrap;gap:.3em 1em;list-style:none;margin:.2em 0;padding:0}\
         .aidoc-relations a{color:#2563eb;text-decoration:none}.aidoc-relations a:hover{text-decoration:underline}\
         h1,h2,h3,h4,h5,h6{margin-top:1.6em}\
         pre{background:#f0f0f0;padding:.75em 1em;border-radius:4px;overflow:auto}\
         blockquote{border-left:4px solid #ccc;margin:0;padding-left:1em;color:#555}\
         [data-aidoc-type=\"requirement\"]{border-left:4px solid #2563eb;background:rgba(37,99,235,.07);padding:.5em 1em}\
         [data-aidoc-type=\"decision\"]{border-left:4px solid #16a34a;background:rgba(22,163,74,.07);padding:.5em 1em}\
         [data-aidoc-type=\"problem\"]{border-left:4px solid #dc2626;background:rgba(220,38,38,.07);padding:.5em 1em}\
         [data-aidoc-type=\"solution\"]{border-left:4px solid #f59e0b;background:rgba(245,158,11,.07);padding:.5em 1em}\
         @media (prefers-color-scheme: dark){\
           body{color:#e6e6e6}\
           .aidoc-toc{background:#1f1f1f;border-color:#333}\
           .aidoc-toc a{color:#bbb}\
           .aidoc-toc a:hover{color:#60a5fa}\
           .aidoc-diagram{background:#262626}\
           .aidoc-branch{background:#2a2f44;border-left-color:#5b6dab}\
           .aidoc-relations{color:#aaa;border-left-color:#444}.aidoc-relations a{color:#60a5fa}\
           pre{background:#1e1e1e;color:#d4d4d4}\
           blockquote{border-left-color:#444;color:#aaa}\
           [data-aidoc-type=\"requirement\"]{border-left-color:#3b82f6;background:rgba(59,130,246,.12)}\
           [data-aidoc-type=\"decision\"]{border-left-color:#22c55e;background:rgba(34,197,94,.12)}\
           [data-aidoc-type=\"problem\"]{border-left-color:#ef4444;background:rgba(239,68,68,.12)}\
           [data-aidoc-type=\"solution\"]{border-left-color:#f59e0b;background:rgba(245,158,11,.12)}\
         }\
         </style>\n",
    );
    out.push_str("</head>\n<body>\n");

    if let Some(b) = branch {
        let _ = writeln!(out, "<aside class=\"aidoc-branch\">{}</aside>", escape(b));
    }

    let by_parent = group_by_parent(nodes);

    // Build a nested table of contents from Section (non-root) + Heading nodes.
    if let Some(root) = by_parent.get(&None).and_then(|r| {
        r.iter()
            .find(|n| n.id == doc.root_node)
            .or_else(|| r.first())
    }) {
        let mut toc = String::new();
        toc.push_str("<nav class=\"aidoc-toc\" aria-label=\"Table of contents\"><details open><summary>Contents</summary>");
        build_toc(&mut toc, root, &by_parent, 1);
        toc.push_str("</details></nav>\n");
        out.push_str(&toc);
    }
    if let Some(root) = by_parent.get(&None) {
        // root is the unique top-level node (matches doc.root_node).
        if let Some(root_node) = root
            .iter()
            .find(|n| n.id == doc.root_node)
            .or_else(|| root.first())
        {
            emit_node(&mut out, root_node, &by_parent, nodes, relations, 1);
        }
    }

    out.push_str("</body>\n</html>\n");
    out
}

/// Walk the tree and emit a nested `<ul>` of (Section non-root) + Heading links.
/// `depth` is the rendering depth; the first call uses 1 (root is depth 1).
fn build_toc(out: &mut String, parent: &Node, by_parent: &Group<'_>, depth: usize) {
    let children = match by_parent.get(&Some(parent.id.clone())) {
        Some(c) => c,
        None => return,
    };
    let mut has_entries = false;
    let mut buf = String::new();
    for c in children {
        let include = matches!(c.kind, NodeKind::Section | NodeKind::Heading)
            // Skip the root section (it's the document title).
            && !(depth == 1 && c.id == parent.id);
        if include {
            has_entries = true;
            let _ = writeln!(
                buf,
                "<li><a href=\"#{}\">{}</a>",
                escape(c.id.as_str()),
                escape(if c.content.is_empty() {
                    c.id.as_str()
                } else {
                    &c.content
                })
            );
            // Recurse into children for nested lists.
            build_toc(&mut buf, c, by_parent, depth + 1);
            buf.push_str("</li>\n");
        } else {
            // Section node we don't want as an entry, but still recurse for descendants.
            build_toc(&mut buf, c, by_parent, depth + 1);
        }
    }
    if has_entries {
        let _ = writeln!(out, "<ul>");
        out.push_str(&buf);
        out.push_str("</ul>\n");
    } else {
        // Forward any pure-nested entries even when this level has none of its own.
        out.push_str(&buf);
    }
}

fn emit_relations(
    out: &mut String,
    node: &Node,
    all: &[Node],
    relations: &[Relation],
    depth: usize,
) {
    let relevant: Vec<_> = relations
        .iter()
        .filter_map(|rel| {
            let (direction, other) = if rel.source == node.id {
                ("out", &rel.target)
            } else if rel.target == node.id {
                ("in", &rel.source)
            } else {
                return None;
            };
            let kind = rel
                .custom_kind
                .as_deref()
                .filter(|_| rel.kind.as_str() == "custom")
                .unwrap_or(rel.kind.as_str());
            let label = all
                .iter()
                .find(|n| n.id == *other)
                .map(|n| n.content.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or(other.as_str());
            Some((direction, kind, other.as_str(), label))
        })
        .collect();
    if relevant.is_empty() {
        return;
    }
    let indent = "  ".repeat(depth);
    let _ = writeln!(
        out,
        "{indent}<aside class=\"aidoc-relations\" aria-label=\"Relations for {}\"><strong>Relations</strong><ul>",
        escape(node.id.as_str())
    );
    for (direction, kind, target, label) in relevant {
        let arrow = if direction == "out" { "→" } else { "←" };
        let _ = writeln!(
            out,
            "{indent}  <li><span>{arrow} {}</span> <a href=\"#{}\">{}</a></li>",
            escape(kind),
            escape(target),
            escape(label)
        );
    }
    let _ = writeln!(out, "{indent}</ul></aside>");
}

type Group<'a> = HashMap<Option<aidoc_model::id::NodeId>, Vec<&'a Node>>;

fn group_by_parent(nodes: &[Node]) -> Group<'_> {
    let mut map: Group = HashMap::new();
    for n in nodes {
        map.entry(n.parent.clone()).or_default().push(n);
    }
    // sort children by (position, id)
    for v in map.values_mut() {
        v.sort_by_key(|n| (n.position, n.id.as_str().to_owned()));
    }
    map
}

fn emit_node(
    out: &mut String,
    node: &Node,
    by_parent: &Group<'_>,
    all: &[Node],
    relations: &[Relation],
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    let id_attr = format!(" id=\"{}\"", escape(node.id.as_str()));
    let sem_attr = node
        .semantic_type
        .as_ref()
        .map(|s| format!(" data-aidoc-type=\"{}\"", escape(s)))
        .unwrap_or_default();

    match node.kind {
        NodeKind::Section => {
            // The root node doubles as the document title (h1) — we render it
            // as <h1> rather than nesting <section>.
            if depth == 1 {
                let _ = writeln!(
                    out,
                    "{}<h1{}>{}</h1>",
                    indent,
                    id_attr,
                    escape(&node.content)
                );
                emit_children(out, node, by_parent, all, relations, depth + 1);
            } else {
                let _ = writeln!(out, "{}<section{}{}>", indent, id_attr, sem_attr);
                if !node.content.is_empty() {
                    let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
                }
                emit_children(out, node, by_parent, all, relations, depth + 1);
                let _ = writeln!(out, "{}</section>", indent);
            }
        }
        NodeKind::Paragraph => {
            let _ = writeln!(out, "{}<p{}>{}</p>", indent, id_attr, escape(&node.content));
        }
        NodeKind::Heading => {
            // Depth 1 is h2 (h1 reserved for root).
            let level = (depth + 1).clamp(2, 6);
            let _ = writeln!(
                out,
                "{}<h{}{}>{}</h{}>",
                indent,
                level,
                id_attr,
                escape(&node.content),
                level
            );
        }
        NodeKind::Code => {
            let _ = writeln!(
                out,
                "{}<pre{}><code>{}</code></pre>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::Blockquote => {
            let _ = writeln!(
                out,
                "{}<blockquote{}>{}</blockquote>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::List => {
            let _ = writeln!(out, "{}<ul{}>", indent, id_attr);
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</ul>", indent);
        }
        NodeKind::ListItem => {
            let _ = writeln!(
                out,
                "{}<li{}>{}</li>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::Table => {
            let _ = writeln!(out, "{}<table{}>", indent, id_attr);
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</table>", indent);
        }
        NodeKind::TableRow => {
            let _ = write!(out, "{}<tr{}>", indent, id_attr);
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</tr>", indent);
        }
        NodeKind::TableCell => {
            let _ = writeln!(
                out,
                "{}<td{}>{}</td>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::Link => {
            let href = node.attributes.get("href").cloned().unwrap_or_default();
            let _ = writeln!(
                out,
                "{}<a href=\"{}\"{}>{}</a>",
                indent,
                escape(&href),
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::Image => {
            let src = node
                .attributes
                .get("src")
                .cloned()
                .unwrap_or_else(|| node.content.clone());
            let _ = writeln!(
                out,
                "{}<img src=\"{}\" alt=\"{}\"{}>",
                indent,
                escape(&src),
                escape(&node.content),
                id_attr
            );
        }
        NodeKind::Diagram => {
            let engine = node
                .attributes
                .get("engine")
                .cloned()
                .unwrap_or_else(|| "mermaid".into());
            let dtype = node
                .attributes
                .get("type")
                .cloned()
                .unwrap_or_else(|| "flowchart".into());
            let _ = writeln!(
                out,
                "{}<diagram{}{} engine=\"{}\" type=\"{}\">",
                indent,
                id_attr,
                sem_attr,
                escape(&engine),
                escape(&dtype)
            );
            let _ = writeln!(
                out,
                "{}  <pre class=\"aidoc-diagram\">{}</pre>",
                indent,
                escape(&node.content)
            );
            let _ = writeln!(out, "{}</diagram>", indent);
        }
        NodeKind::CodeRef => {
            let file = node.attributes.get("file").cloned().unwrap_or_default();
            let symbol = node.attributes.get("symbol").cloned().unwrap_or_default();
            let _ = writeln!(
                out,
                "{}<code-ref{} file=\"{}\" symbol=\"{}\">",
                indent,
                id_attr,
                escape(&file),
                escape(&symbol)
            );
            if !node.content.is_empty() {
                let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            }
            let _ = writeln!(out, "{}</code-ref>", indent);
        }
        NodeKind::Requirement => {
            let _ = writeln!(out, "{}<requirement{}>", indent, id_attr);
            let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            let _ = writeln!(out, "{}</requirement>", indent);
        }
        NodeKind::Decision => {
            let _ = writeln!(out, "{}<decision{}>", indent, id_attr);
            let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            let _ = writeln!(out, "{}</decision>", indent);
        }
        NodeKind::Problem => {
            let _ = writeln!(out, "{}<problem{}>", indent, id_attr);
            let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            let _ = writeln!(out, "{}</problem>", indent);
        }
        NodeKind::Solution => {
            let _ = writeln!(out, "{}<solution{}>", indent, id_attr);
            let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            let _ = writeln!(out, "{}</solution>", indent);
        }
        NodeKind::Reference => {
            let target = node.attributes.get("target").cloned().unwrap_or_default();
            let kind = node.attributes.get("relation").cloned().unwrap_or_default();
            let _ = writeln!(
                out,
                "{}<ref{} target=\"{}\" relation=\"{}\">{}</ref>",
                indent,
                id_attr,
                escape(&target),
                escape(&kind),
                escape(&node.content)
            );
        }
        NodeKind::Details => {
            let _ = writeln!(out, "{}<details{}>", indent, id_attr);
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</details>", indent);
        }
        NodeKind::Summary => {
            let _ = writeln!(
                out,
                "{}<summary{}>{}</summary>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
        NodeKind::Generic => {
            let _ = writeln!(
                out,
                "{}<div{}>{}</div>",
                indent,
                id_attr,
                escape(&node.content)
            );
        }
    }
    emit_relations(out, node, all, relations, depth);
}

fn emit_children(
    out: &mut String,
    parent: &Node,
    by_parent: &Group<'_>,
    _all: &[Node],
    relations: &[Relation],
    depth: usize,
) {
    if let Some(children) = by_parent.get(&Some(parent.id.clone())) {
        for c in children {
            emit_node(out, c, by_parent, _all, relations, depth);
        }
    }
}

fn escape(s: &str) -> String {
    let mut buf = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '&' => buf.push_str("&amp;"),
            '"' => buf.push_str("&quot;"),
            '\'' => buf.push_str("&#39;"),
            _ => buf.push(c),
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use aidoc_model::{Document, Node, RelationKind, id::NodeId};

    fn make_node(id: &str, kind: NodeKind, content: &str) -> Node {
        let mut n = Node::new(NodeId::from_validated(id), kind);
        n.content = content.to_string();
        n
    }

    #[test]
    fn renders_minimal_doc() {
        let doc = Document::new("d1", "Demo", NodeId::from_validated("root"));
        let root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        let html = render_html(&doc, &[root], &[], None);
        assert!(html.contains("<h1"));
        assert!(html.contains("Demo"));
        assert!(html.ends_with("</html>\n"));
    }

    #[test]
    fn toc_contains_nested_sections_and_headings() {
        let doc = Document::new("d1", "Doc", NodeId::from_validated("root"));
        let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        root.content = "Doc".into();
        let mut sec1 = make_node("s1", NodeKind::Section, "Section One");
        sec1.parent = Some(NodeId::from_validated("root"));
        let mut h1 = make_node("h1", NodeKind::Heading, "Heading One");
        h1.parent = Some(NodeId::from_validated("s1"));
        let mut sec2 = make_node("s2", NodeKind::Section, "Section Two");
        sec2.parent = Some(NodeId::from_validated("root"));

        let html = render_html(&doc, &[root, sec1, h1, sec2], &[], None);
        assert!(html.contains("<nav class=\"aidoc-toc\""), "toc nav missing");
        assert!(html.contains("href=\"#s1\""));
        assert!(html.contains("href=\"#h1\""));
        assert!(html.contains("href=\"#s2\""));
        // Nested: <h1> is under s1, so its <li> must sit inside s1's <ul>.
        let s1_open = html.find("href=\"#s1\"").unwrap();
        let h1_pos = html.find("href=\"#h1\"").unwrap();
        assert!(
            h1_pos > s1_open,
            "h1 link should appear after s1 link (nested)"
        );
    }

    #[test]
    fn toc_skips_root_section() {
        let doc = Document::new("d1", "Title", NodeId::from_validated("root"));
        let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        root.content = "Title".into();
        let html = render_html(&doc, &[root], &[], None);
        // The root section IS rendered as <h1>, but it must NOT also be a TOC entry.
        assert!(
            html.contains("href=\"#root\"") == false,
            "root leaked into TOC"
        );
    }

    #[test]
    fn renders_incoming_and_outgoing_relations_as_links() {
        let doc = Document::new("d1", "Doc", NodeId::from_validated("root"));
        let root = make_node("root", NodeKind::Section, "Doc");
        let mut target = make_node("target", NodeKind::Paragraph, "Target title");
        target.parent = Some(root.id.clone());
        let relation = Relation {
            id: "rel-1".into(),
            source: root.id.clone(),
            target: target.id.clone(),
            kind: RelationKind::DependsOn,
            custom_kind: None,
        };
        let html = render_html(&doc, &[root.clone(), target], &[relation], None);
        assert!(html.contains("→ depends-on"));
        assert!(html.contains("← depends-on"));
        assert!(html.contains("href=\"#target\">Target title</a>"));
    }
}
