//! HTML / Markdown export for an open `.aidoc` package.

use std::collections::HashMap;
use std::fmt::Write as _;

use aidoc_model::{Document, Node, NodeKind};

pub fn export_html(doc: &Document, nodes: &[Node]) -> String {
    aidoc_renderer::render_html(doc, nodes, &[])
}

pub fn export_markdown(doc: &Document, nodes: &[Node]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", doc.title);
    let by_parent = group_by_parent(nodes);
    let root_id = doc.root_node.clone();
    if let Some(roots) = by_parent.get(&None) {
        let top = roots.iter().find(|n| n.id == root_id).or_else(|| roots.first());
        if let Some(root) = top {
            emit_md(&mut out, root, &by_parent, 1);
        }
    }
    out
}

type Group<'a> = HashMap<Option<aidoc_model::id::NodeId>, Vec<&'a Node>>;

fn group_by_parent(nodes: &[Node]) -> Group<'_> {
    let mut m: Group = HashMap::new();
    for n in nodes {
        m.entry(n.parent.clone()).or_default().push(n);
    }
    for v in m.values_mut() {
        v.sort_by_key(|n| (n.position, n.id.as_str().to_owned()));
    }
    m
}

fn emit_md(out: &mut String, node: &Node, by_parent: &Group<'_>, depth: usize) {
    let hashes = "#".repeat((depth + 1).min(6));
    let content = node.content.trim();
    match node.kind {
        NodeKind::Section => {
            if !content.is_empty() {
                let _ = writeln!(out, "{hashes} {content}\n");
            }
        }
        NodeKind::Heading => {
            let _ = writeln!(out, "{hashes} {content}\n");
        }
        NodeKind::Paragraph => {
            if !content.is_empty() {
                let _ = writeln!(out, "{content}\n");
            }
        }
        NodeKind::Code => {
            let _ = writeln!(out, "```\n{content}\n```\n");
        }
        NodeKind::Blockquote => {
            for line in content.lines() {
                let _ = writeln!(out, "> {line}");
            }
            out.push('\n');
        }
        NodeKind::List | NodeKind::ListItem => {
            let _ = writeln!(out, "- {content}");
        }
        NodeKind::Table | NodeKind::TableRow | NodeKind::TableCell => {
            let _ = writeln!(out, "| {content} |");
        }
        NodeKind::Link => {
            let href = node.attributes.get("href").cloned().unwrap_or_default();
            let _ = writeln!(out, "[{content}]({href})");
        }
        NodeKind::Image => {
            let src = node.attributes.get("src").cloned().unwrap_or_default();
            let _ = writeln!(out, "![{content}]({src})");
        }
        NodeKind::Diagram => {
            let _ = writeln!(out, "```{}\n{}\n```\n", "mermaid", content);
        }
        NodeKind::CodeRef => {
            let file = node.attributes.get("file").cloned().unwrap_or_default();
            let symbol = node.attributes.get("symbol").cloned().unwrap_or_default();
            let _ = writeln!(out, "**`{symbol}`** in `{file}`");
            if !content.is_empty() {
                let _ = writeln!(out, "\n{content}\n");
            }
        }
        NodeKind::Requirement => {
            let _ = writeln!(out, "> **Requirement** — {content}\n");
        }
        NodeKind::Decision => {
            let _ = writeln!(out, "> **Decision** — {content}\n");
        }
        NodeKind::Problem => {
            let _ = writeln!(out, "> **Problem** — {content}\n");
        }
        NodeKind::Solution => {
            let _ = writeln!(out, "> **Solution** — {content}\n");
        }
        NodeKind::Reference => {
            let _ = writeln!(out, "[ref]({})", node.attributes.get("target").cloned().unwrap_or_default());
        }
        NodeKind::Details | NodeKind::Summary | NodeKind::Generic => {
            if !content.is_empty() {
                let _ = writeln!(out, "{content}\n");
            }
        }
    }

    if let Some(children) = by_parent.get(&Some(node.id.clone())) {
        for c in children {
            emit_md(out, c, by_parent, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aidoc_model::id::NodeId;

    #[test]
    fn exports_md_title() {
        let doc = Document::new("d1", "Demo Title", NodeId::from_validated("root"));
        let out = export_markdown(&doc, &[]);
        assert!(out.contains("# Demo Title"));
    }
}