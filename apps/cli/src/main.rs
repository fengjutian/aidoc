//! `aidoc` CLI.

mod commands;
mod session;

use clap::{Parser, Subcommand};

use anyhow::Result;

#[derive(Parser)]
#[command(
    name = "aidoc",
    version,
    about = "AIDoc v0.1 — AI-native structured document format"
)]
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
        /// Filter to a named branch (R### revisions on that branch only).
        #[arg(long)]
        branch: Option<String>,
    },

    /// Show a diff between two revisions (R### IDs).
    ///
    /// For v0.1 this prints the node contents at both revisions side-by-side.
    Diff {
        path: String,
        /// Source revision (R###). Defaults to the revision immediately before `to`.
        #[arg(default_value = None)]
        from: Option<String>,
        /// Target revision (R###). Defaults to the current head.
        #[arg(default_value = None)]
        to: Option<String>,
        /// Restrict both `from` and `to` to revisions on this named branch.
        /// When set, `from` defaults to the previous revision on the branch.
        #[arg(long)]
        branch: Option<String>,
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
    Validate {
        path: String,
        /// Repository path for code-ref staleness checks (spec §37). Omit to
        /// skip git and run the hermetic structural validators only.
        #[arg(long)]
        repo: Option<String>,
    },

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

    /// Scaffold one of the bundled example documents.
    ///
    /// `aidoc example order-system examples/order-system.aidoc`
    /// writes a brand-new `.aidoc` package pre-populated with the order-system
    /// demo tree. Currently bundled:
    ///   - order-system
    ///   - api-system
    ///   - knowledge-graph
    Example {
        /// Template name.
        template: String,
        /// Destination path.
        path: String,
        /// Also export the resulting HTML here.
        #[arg(long)]
        export_html: Option<String>,
    },

    /// Tag the current head with a named branch (spec §34).
    ///
    /// `aidoc branch path/to/doc.aidoc ai-draft`
    /// creates a new revision labelled "ai-draft". Nodes are not mutated;
    /// subsequent ops against the new head belong to that branch.
    Branch {
        path: String,
        /// Branch name. Must be non-empty and not "main".
        name: String,
        #[arg(long)]
        reason: Option<String>,
    },

    /// Switch to the tip of main or a named branch.
    Checkout {
        path: String,
        branch: String,
    },

    /// Three-way merge a named branch into the checked-out main branch.
    Merge {
        path: String,
        /// Source branch to merge.
        branch: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let res = match cli.cmd {
        Cmd::Init {
            path,
            doc_id,
            title,
        } => commands::init::run(&path, doc_id, title),
        Cmd::Info { path } => commands::info::run(&path),
        Cmd::NodeList { path, json } => commands::node_list::run(&path, json),
        Cmd::NodeShow {
            path,
            node_id,
            json,
        } => commands::node_show::run(&path, &node_id, json),
        Cmd::Apply {
            path,
            op_file,
            print_revision,
        } => commands::apply::run(&path, op_file.as_deref(), print_revision),
        Cmd::History { path, json, branch } => {
            commands::history::run(&path, json, branch.as_deref())
        }
        Cmd::Diff {
            path,
            from,
            to,
            branch,
        } => commands::diff::run(&path, from.as_deref(), to.as_deref(), branch.as_deref()),
        Cmd::Revert {
            path,
            target,
            reason,
        } => commands::revert::run(&path, &target, reason.as_deref()),
        Cmd::Validate { path, repo } => commands::validate::run(&path, repo.as_deref()),
        Cmd::Export { path, out, format } => commands::export::run(&path, &out, &format),
        Cmd::Demo { path, export_html } => commands::demo::run(&path, export_html.as_deref()),
        Cmd::Example {
            template,
            path,
            export_html,
        } => commands::example::run(&template, &path, export_html.as_deref()),
        Cmd::Branch { path, name, reason } => {
            commands::branch::run(&path, &name, reason.as_deref())
        }
        Cmd::Checkout { path, branch } => commands::checkout::run(&path, &branch),
        Cmd::Merge {
            path,
            branch,
            reason,
        } => commands::merge::run(&path, &branch, reason.as_deref()),
    };

    // `validate` wants to communicate the error count via the process exit
    // code so CI pipelines can read it. Capped at 125 because Unix shells
    // only use 8 bits for $? and 126/127 are reserved for exec / not-found.
    if let Err(e) = res {
        if let Some(failure) = e.downcast_ref::<commands::validate::ValidateFailure>() {
            let code = failure.count.clamp(1, 125) as i32;
            std::process::exit(code);
        }
        return Err(e);
    }
    Ok(())
}
