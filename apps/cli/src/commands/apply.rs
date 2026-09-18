use std::io::Read;

use anyhow::{Context, Result};

use aidoc::{ApplyError, Operation, apply_operation};

use crate::session::Session;

pub fn run(path: &str, op_file: Option<&str>, print_revision: bool) -> Result<()> {
    let op: Operation = match op_file {
        Some(f) => {
            let raw = std::fs::read_to_string(f).context("read op file")?;
            serde_json::from_str(&raw).context("parse op file")?
        }
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("read stdin")?;
            serde_json::from_str(&buf).context("parse stdin JSON")?
        }
    };

    let mut s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let outcome = match apply_operation(&mut s.store, &doc_id, op) {
        Ok(o) => o,
        // §25/§36: print the machine-readable conflict shape to stderr so a
        // caller (or an agent) can react to it, then fail with a non-zero exit.
        Err(ApplyError::Conflict(c)) => {
            let json = serde_json::to_string_pretty(&c.to_json())?;
            eprintln!("{json}");
            return Err(anyhow::anyhow!("{c}"));
        }
        Err(e) => return Err(e.into()),
    };

    // bump manifest head + updated_at, then save
    if let Some(pkg) = s.package.as_mut() {
        pkg.manifest.set_revision(outcome.revision.as_str());
    }
    s.save()?;

    if print_revision {
        println!("{}", outcome.revision.as_str());
    } else {
        println!(
            "applied {} -> new revision {}",
            outcome.op_id.as_str(),
            outcome.revision.as_str()
        );
    }
    Ok(())
}
