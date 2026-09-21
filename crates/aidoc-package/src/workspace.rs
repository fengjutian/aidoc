//! On-disk workspace for an opened `.aidoc` package.
//!
//! Layout:
//!   <tmpdir>/
//!     manifest.json
//!     document/document.html
//!     .internal/document.db
//!     assets/        (mirrored as-is)

use std::path::{Path, PathBuf};

use thiserror::Error;

use aidoc_model::{Node, NodeKind};
use aidoc_storage::Store;

use crate::manifest::Manifest;
use crate::zip_io;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("package missing required file: {0}")]
    MissingRequired(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}

/// Open a `.aidoc` file into a temporary workspace + a connected Store.
pub fn open_package(path: impl AsRef<Path>) -> Result<(Package, Store), PackageError> {
    let path = path.as_ref();
    let temp = tempdir()?;
    zip_io::extract_zip(path, temp.path())?;

    let manifest_path = temp.path().join("manifest.json");
    if !manifest_path.exists() {
        return Err(PackageError::MissingRequired("manifest.json".into()));
    }
    let manifest_raw = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_raw)
        .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;

    let db_path = temp.path().join(&manifest.storage.path);
    if !db_path.exists() {
        return Err(PackageError::MissingRequired(manifest.storage.path.clone()));
    }

    let store = Store::open(&db_path)?;

    Ok((
        Package {
            workspace: temp,
            manifest,
            source_path: path.to_path_buf(),
        },
        store,
    ))
}

/// Create a brand-new empty workspace + DB for `init`.
pub fn create_package(
    out_path: impl AsRef<Path>,
    doc_id: impl Into<String> + Clone,
    title: impl Into<String> + Clone,
) -> Result<(Package, Store), PackageError> {
    let temp = tempdir()?;
    let manifest = Manifest::new(doc_id.clone(), title.clone(), "R000");

    // Create dirs.
    std::fs::create_dir_all(temp.path().join("document"))?;
    std::fs::create_dir_all(temp.path().join(".internal"))?;
    std::fs::create_dir_all(temp.path().join("assets"))?;

    // Write manifest.
    std::fs::write(
        temp.path().join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // Write empty HTML.
    let title_str: String = title.into();
    std::fs::write(
        temp.path().join("document/document.html"),
        minimal_html(&title_str),
    )?;

    // Open an empty SQLite db.
    let db_path = temp.path().join(".internal/document.db");
    let store = Store::open(&db_path)?;

    Ok((
        Package {
            workspace: temp,
            manifest,
            source_path: out_path.as_ref().to_path_buf(),
        },
        store,
    ))
}

/// Save the current workspace back to its source `.aidoc` ZIP.
pub fn save_package(pkg: &mut Package, store: &Store) -> Result<(), PackageError> {
    if let Some(head) = aidoc_storage::crud::head_revision(store.conn(), &pkg.manifest.document.id)?
    {
        pkg.manifest.set_revision(head);
    } else {
        pkg.manifest.touch_updated();
    }
    std::fs::write(
        pkg.workspace.path().join("manifest.json"),
        serde_json::to_string_pretty(&pkg.manifest)?,
    )?;
    zip_io::pack_zip(pkg.workspace.path(), &pkg.source_path)?;
    Ok(())
}

fn minimal_html(title: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head><meta charset=\"utf-8\"><title>{title}</title></head>\n<body>\n<h1 id=\"root\">{title}</h1>\n</body>\n</html>\n"
    )
}

// tiny wrapper around tempfile::TempDir so we don't expose the dep in our public API
pub struct TempDir(tempfile::TempDir);
impl TempDir {
    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

fn tempdir() -> std::io::Result<TempDir> {
    Ok(TempDir(
        tempfile::Builder::new().prefix("aidoc-").tempdir()?,
    ))
}

/// Live handle to a workspace extracted from a `.aidoc` file.
pub struct Package {
    pub workspace: TempDir,
    pub manifest: Manifest,
    pub source_path: PathBuf,
}

impl Package {
    pub fn workspace_path(&self) -> &Path {
        self.workspace.path()
    }
}

/// Replace package-relative image sources with data URLs in an export-only
/// node copy. Live nodes keep their compact `assets/...` references.
pub fn inline_image_assets(pkg: &Package, nodes: &mut [Node]) -> Result<(), PackageError> {
    for node in nodes.iter_mut().filter(|n| n.kind == NodeKind::Image) {
        let source = node
            .attributes
            .get("src")
            .cloned()
            .unwrap_or_else(|| node.content.clone());
        if !source.starts_with("assets/") || source.contains("..") {
            continue;
        }
        let path = pkg.workspace_path().join(&source);
        let mime = match path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            other => {
                return Err(PackageError::InvalidManifest(format!(
                    "unsupported image extension: {other}"
                )));
            }
        };
        let bytes = std::fs::read(path)?;
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = ((chunk[0] as u32) << 16)
                | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
                | chunk.get(2).copied().unwrap_or(0) as u32;
            encoded.push(TABLE[((n >> 18) & 63) as usize] as char);
            encoded.push(TABLE[((n >> 12) & 63) as usize] as char);
            encoded.push(if chunk.len() > 1 {
                TABLE[((n >> 6) & 63) as usize] as char
            } else {
                '='
            });
            encoded.push(if chunk.len() > 2 {
                TABLE[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        let data_url = format!("data:{mime};base64,{encoded}");
        node.content = data_url.clone();
        node.attributes.insert("src".into(), data_url);
    }
    Ok(())
}

#[cfg(test)]
mod asset_tests {
    use super::*;
    use aidoc_model::{NodeId, NodeKind};

    #[test]
    fn export_copy_inlines_packaged_image_without_mutating_package() {
        let dir = tempfile::tempdir().unwrap();
        let (pkg, _store) = create_package(dir.path().join("x.aidoc"), "x", "X").unwrap();
        std::fs::write(pkg.workspace_path().join("assets/p.png"), b"foo").unwrap();
        let mut image = Node::new(NodeId::from_validated("image"), NodeKind::Image);
        image.content = "assets/p.png".into();
        inline_image_assets(&pkg, std::slice::from_mut(&mut image)).unwrap();
        assert_eq!(image.content, "data:image/png;base64,Zm9v");
        assert!(pkg.workspace_path().join("assets/p.png").exists());
    }
}
