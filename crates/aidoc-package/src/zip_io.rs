//! Minimal zip read/write helpers (pure std + `zip` crate).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub fn extract_zip(zip_path: &Path, dest: &Path) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_path = entry
            .enclosed_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad zip entry name"))?;
        let out_path = dest.join(entry_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

pub fn pack_zip(src_dir: &Path, zip_path: &Path) -> io::Result<()> {
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Build beside the destination so a failed write never truncates the
    // existing package and the final rename stays on the same filesystem.
    let parent = zip_path.parent().unwrap_or_else(|| Path::new("."));
    let file = tempfile::NamedTempFile::new_in(parent)?;
    let (file, temp_path) = file.keep().map_err(|e| e.error)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(src_dir).into_iter() {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();

    for path in &files {
        let rel = path.strip_prefix(src_dir).unwrap();
        let name = rel.to_string_lossy().replace('\\', "/");
        zip.start_file(&name, opts)?;
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;
    }
    zip.finish()?;
    std::fs::rename(temp_path, zip_path)?;
    Ok(())
}
