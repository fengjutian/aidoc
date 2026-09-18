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

use aidoc_model::{
    Document, Node, NodeKind, Relation,
};

pub fn render_html(doc: &Document, nodes: &[Node], relations: &[Relation]) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    let _ = write!(
        out,
        "  <meta charset=\"utf-8\">\n  <title>{}</title>\n",
        escape(&doc.title)
    );
    out.push_str("  <style>.aidoc-diagram{font-family:monospace;background:#f8f8f8;padding:.5em}</style>\n");
    out.push_str("</head>\n<body>\n");

    let by_parent = group_by_parent(nodes);
    if let Some(root) = by_parent.get(&None) {
        // root is the unique top-level node (matches doc.root_node).
        if let Some(root_node) = root.iter().find(|n| n.id == doc.root_node).or_else(|| root.first()) {
            emit_node(&mut out, root_node, &by_parent, nodes, relations, 1);
        }
    }

    out.push_str("</body>\n</html>\n");
    out
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
            let _ = write!(out, "{}<section{}{}>\n", indent, id_attr, sem_attr);
            if !node.content.is_empty() {
                let _ = writeln!(out, "{}  <p>{}</p>", indent, escape(&node.content));
            }
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</section>", indent);
        }
        NodeKind::Paragraph => {
            let _ = writeln!(out, "{}<p{}>{}</p>", indent, id_attr, escape(&node.content));
        }
        NodeKind::Heading => {
            // Depth 1 is h2 (h1 reserved for root).
            let level = (depth + 1).min(6).max(2);
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
            let _ = writeln!(out, "{}<pre{}><code>{}</code></pre>", indent, id_attr, escape(&node.content));
        }
        NodeKind::Blockquote => {
            let _ = writeln!(out, "{}<blockquote{}>{}</blockquote>", indent, id_attr, escape(&node.content));
        }
        NodeKind::List => {
            let _ = writeln!(out, "{}<ul{}>", indent, id_attr);
            emit_children(out, node, by_parent, all, relations, depth + 1);
            let _ = writeln!(out, "{}</ul>", indent);
        }
        NodeKind::ListItem => {
            let _ = writeln!(out, "{}<li{}>{}</li>", indent, id_attr, escape(&node.content));
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
            let _ = writeln!(out, "{}<td{}>{}</td>", indent, id_attr, escape(&node.content));
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
            let src = node.attributes.get("src").cloned().unwrap_or_default();
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
            let engine = node.attributes.get("engine").cloned().unwrap_or_else(|| "mermaid".into());
            let dtype = node.attributes.get("type").cloned().unwrap_or_else(|| "flowchart".into());
            let _ = writeln!(
                out,
                "{}<diagram{}{} engine=\"{}\" type=\"{}\">",
                indent, id_attr, sem_attr, escape(&engine), escape(&dtype)
            );
            let _ = writeln!(out, "{}  <pre class=\"aidoc-diagram\">{}</pre>", indent, escape(&node.content));
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
            let _ = writeln!(out, "{}<summary{}>{}</summary>", indent, id_attr, escape(&node.content));
        }
        NodeKind::Generic => {
            let _ = writeln!(out, "{}<div{}>{}</div>", indent, id_attr, escape(&node.content));
        }
    }
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
    let _ = relations; // reserved for future "see also" rendering
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
    use aidoc_model::{id::NodeId, Document, Node};

    #[test]
    fn renders_minimal_doc() {
        let doc = Document::new("d1", "Demo", NodeId::from_validated("root"));
        let root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        let html = render_html(&doc, &[root], &[]);
        assert!(html.contains("<h1"));
        assert!(html.contains("Demo"));
        assert!(html.ends_with("</html>\n"));
    }
}