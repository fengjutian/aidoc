use anyhow::Result;

use crate::session::Session;

pub fn run(path: &str) -> Result<()> {
    let s = Session::open(path)?;
    let m = &s.package.as_ref().unwrap().manifest;
    println!("format      : {}", m.format);
    println!("version     : {}", m.version);
    println!("doc id      : {}", m.document.id);
    println!("title       : {}", m.document.title);
    println!("entry       : {}", m.entry);
    println!("storage     : {} @ {}", m.storage.kind, m.storage.path);
    println!("head rev    : {}", m.revision.current);
    println!("created     : {}", m.created_at.to_rfc3339());
    println!("updated     : {}", m.updated_at.to_rfc3339());
    let _ = s; // drop
    Ok(())
}