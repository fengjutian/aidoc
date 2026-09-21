//! HTML / Markdown export for an open `.aidoc` package (spec §41-§43).
//!
//! Export is pluggable: each format implements the [`Exporter`] trait and is
//! resolved by [`exporter_for`]. Adding DOCX / PDF later means a new
//! [`ExportFormat`] variant + an [`Exporter`] impl, without touching callers —
//! they keep going through [`export`].

use std::collections::HashMap;
use std::fmt::Write as _;

use aidoc_model::{Document, Node, NodeKind, Relation};

/// Supported export formats (spec §41).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    Html,
    Markdown,
}

impl ExportFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "md",
        }
    }
}

/// Error returned when a format string is not recognised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownExportFormat(pub String);

impl std::fmt::Display for UnknownExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown format: {} (use html or md)", self.0)
    }
}

impl std::error::Error for UnknownExportFormat {}

/// Parse a user-supplied format string, absorbing the `md` / `markdown`
/// aliases. Implemented as [`std::str::FromStr`] so `"...".parse()` works.
impl std::str::FromStr for ExportFormat {
    type Err = UnknownExportFormat;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "html" => Ok(Self::Html),
            "md" | "markdown" => Ok(Self::Markdown),
            other => Err(UnknownExportFormat(other.to_owned())),
        }
    }
}

/// Everything an [`Exporter`] needs to render a document.
pub struct ExportInput<'a> {
    pub doc: &'a Document,
    pub nodes: &'a [Node],
    pub relations: &'a [Relation],
    pub branch: Option<&'a str>,
}

/// A pluggable exporter for a single [`ExportFormat`].
pub trait Exporter {
    fn format(&self) -> ExportFormat;
    fn export(&self, input: &ExportInput) -> String;
}

/// HTML exporter — delegates to `aidoc-renderer`. Unlike the pre-refactor
/// free function, this threads `input.relations` into the renderer.
pub struct HtmlExporter;

impl Exporter for HtmlExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Html
    }
    fn export(&self, input: &ExportInput) -> String {
        aidoc_renderer::render_html(input.doc, input.nodes, input.relations, input.branch)
    }
}

/// Markdown exporter (spec §43). Lossy by design — AIDoc semantics degrade.
pub struct MarkdownExporter;

impl Exporter for MarkdownExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Markdown
    }
    fn export(&self, input: &ExportInput) -> String {
        render_markdown(input.doc, input.nodes, input.relations)
    }
}

/// Resolve the exporter for a format.
pub fn exporter_for(fmt: ExportFormat) -> Box<dyn Exporter> {
    match fmt {
        ExportFormat::Html => Box::new(HtmlExporter),
        ExportFormat::Markdown => Box::new(MarkdownExporter),
    }
}

/// Dispatch to the exporter registered for `fmt`.
pub fn export(fmt: ExportFormat, input: &ExportInput) -> String {
    exporter_for(fmt).export(input)
}

/// Back-compat wrapper: export HTML (no relations threaded).
pub fn export_html(doc: &Document, nodes: &[Node], branch: Option<&str>) -> String {
    export(
        ExportFormat::Html,
        &ExportInput {
            doc,
            nodes,
            relations: &[],
            branch,
        },
    )
}

/// Back-compat wrapper: export Markdown.
pub fn export_markdown(doc: &Document, nodes: &[Node]) -> String {
    export(
        ExportFormat::Markdown,
        &ExportInput {
            doc,
            nodes,
            relations: &[],
            branch: None,
        },
    )
}

fn render_markdown(doc: &Document, nodes: &[Node], relations: &[Relation]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", doc.title);
    let by_parent = group_by_parent(nodes);
    let root_id = doc.root_node.clone();
    if let Some(roots) = by_parent.get(&None) {
        let top = roots
            .iter()
            .find(|n| n.id == root_id)
            .or_else(|| roots.first());
        if let Some(root) = top {
            emit_md(&mut out, root, &by_parent, nodes, relations, 1);
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

fn emit_md(
    out: &mut String,
    node: &Node,
    by_parent: &Group<'_>,
    all: &[Node],
    relations: &[Relation],
    depth: usize,
) {
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
            let src = node
                .attributes
                .get("src")
                .cloned()
                .unwrap_or_else(|| node.content.clone());
            let _ = writeln!(out, "![{content}]({src})");
        }
        NodeKind::Diagram => {
            let _ = writeln!(out, "```mermaid\n{}\n```", content);
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
            let _ = writeln!(
                out,
                "[ref]({})",
                node.attributes.get("target").cloned().unwrap_or_default()
            );
        }
        NodeKind::Details | NodeKind::Summary | NodeKind::Generic => {
            if !content.is_empty() {
                let _ = writeln!(out, "{content}\n");
            }
        }
    }

    let relevant: Vec<_> = relations
        .iter()
        .filter_map(|rel| {
            let (arrow, other) = if rel.source == node.id {
                ("→", &rel.target)
            } else if rel.target == node.id {
                ("←", &rel.source)
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
            Some((arrow, kind, other.as_str(), label))
        })
        .collect();
    if !relevant.is_empty() {
        let _ = writeln!(out, "**Relations**");
        for (arrow, kind, target, label) in relevant {
            let _ = writeln!(out, "- {arrow} `{kind}` [{label}](#{target})");
        }
        out.push('\n');
    }

    if let Some(children) = by_parent.get(&Some(node.id.clone())) {
        for c in children {
            emit_md(out, c, by_parent, all, relations, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aidoc_model::RelationKind;
    use aidoc_model::id::NodeId;

    #[test]
    fn exports_md_title() {
        let doc = Document::new("d1", "Demo Title", NodeId::from_validated("root"));
        let out = export_markdown(&doc, &[]);
        assert!(out.contains("# Demo Title"));
    }

    #[test]
    fn format_parses_aliases() {
        assert_eq!("html".parse::<ExportFormat>().unwrap(), ExportFormat::Html);
        assert_eq!(
            "md".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert_eq!(
            "markdown".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert!("pdf".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn dispatch_matches_direct_call() {
        let doc = Document::new("d1", "Demo Title", NodeId::from_validated("root"));
        let input = ExportInput {
            doc: &doc,
            nodes: &[],
            relations: &[],
            branch: None,
        };
        assert_eq!(
            export(ExportFormat::Markdown, &input),
            export_markdown(&doc, &[])
        );
    }

    #[test]
    fn markdown_export_includes_relation_links() {
        let doc = Document::new("d1", "Doc", NodeId::from_validated("root"));
        let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        root.content = "Doc".into();
        let mut target = Node::new(NodeId::from_validated("target"), NodeKind::Paragraph);
        target.content = "Target".into();
        target.parent = Some(root.id.clone());
        let relation = Relation {
            id: "r".into(),
            source: root.id.clone(),
            target: target.id.clone(),
            kind: RelationKind::References,
            custom_kind: None,
        };
        let out = export(
            ExportFormat::Markdown,
            &ExportInput {
                doc: &doc,
                nodes: &[root, target],
                relations: &[relation],
                branch: None,
            },
        );
        assert!(out.contains("→ `references` [Target](#target)"));
        assert!(out.contains("← `references` [Doc](#root)"));
    }
}
