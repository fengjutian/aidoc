//! `aidoc` CLI.

mod commands;
mod session;

use clap::{Parser, Subcommand};

use anyhow::Result;

#[derive(Parser)]
#[command(name = "aidoc", version, about = "AIDoc v0.1 — AI-native structured document format")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a brand-new .aidoc file with an empty document.
    Init {
        /// Output path, e.g. `examples/order-system.aidoc`.
        path: String,
        #[arg(long)]
        doc_id: Option<String>,
        #[arg(long)]
        title: Option<String>,
    },

    /// Show the package manifest and document title.
    Info {
        /// Path to the .aidoc file.
        path: String,
    },

    /// List all nodes in the document.
    NodeList {
        path: String,
        #[arg(long)]
        json: bool,
    },

    /// Show details for a single node.
    NodeShow {
        path: String,
        node_id: String,
        #[arg(long)]
        json: bool,
    },

    /// Apply an Operation described by a JSON file (or stdin) to the document.
    Apply {
        path: String,
        /// Path to a JSON operation file. If omitted, reads stdin.
        #[arg(long)]
        op_file: Option<String>,
        /// Print resulting revision to stdout.
        #[arg(long)]
        print_revision: bool,
    },

    /// Print revision history.
    History {
        path: String,
        #[arg(long)]
        json: bool,
    },

    /// Show a diff between two revisions (R### IDs).
    ///
    /// For v0.1 this prints the node contents at both revisions side-by-side.
    Diff {
        path: String,
        from: String,
        to: String,
    },

    /// Revert to a previous revision (always creates a new revision).
    Revert {
        path: String,
        /// Target revision, e.g. R101.
        target: String,
        #[arg(long)]
        reason: Option<String>,
    },

    /// Validate identity / structure / relation / revision / code-ref rules.
    Validate { path: String },

    /// Export the current document to HTML or Markdown.
    Export {
        path: String,
        /// Output file path.
        out: String,
        /// Format: html | md
        #[arg(long, default_value = "html")]
        format: String,
    },

    /// Run a built-in demo: create→update→revert and dump an HTML file.
    Demo {
        /// Output .aidoc file. The CLI will create it and run the full loop.
        path: String,
        /// Where to drop the final exported HTML.
        #[arg(long)]
        export_html: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init { path, doc_id, title } => commands::init::run(&path, doc_id, title),
        Cmd::Info { path } => commands::info::run(&path),
        Cmd::NodeList { path, json } => commands::node_list::run(&path, json),
        Cmd::NodeShow { path, node_id, json } => commands::node_show::run(&path, &node_id, json),
        Cmd::Apply { path, op_file, print_revision } => {
            commands::apply::run(&path, op_file.as_deref(), print_revision)
        }
        Cmd::History { path, json } => commands::history::run(&path, json),
        Cmd::Diff { path, from, to } => commands::diff::run(&path, &from, &to),
        Cmd::Revert { path, target, reason } => {
            commands::revert::run(&path, &target, reason.as_deref())
        }
        Cmd::Validate { path } => commands::validate::run(&path),
        Cmd::Export { path, out, format } => commands::export::run(&path, &out, &format),
        Cmd::Demo { path, export_html } => commands::demo::run(&path, export_html.as_deref()),
    }
}