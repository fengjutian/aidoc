//! End-to-end test for `aidoc validate` exit codes.
//!
//! Spawns the compiled `aidoc` binary against a deliberately-broken package
//! and asserts the process exits non-zero with a code that reflects the
//! number of validation errors (capped at 125).
//!
//! Run with:
//!   cargo test -p aidoc-cli --test validate_exit -- --nocapture

use std::path::PathBuf;
use std::process::Command;

use aidoc_storage::AnyhowErr;

fn binary_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // apps/cli → apps
    p.pop(); // apps → repo root
    p.push("target");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    p.push(profile);
    #[cfg(windows)]
    p.push("aidoc.exe");
    #[cfg(not(windows))]
    p.push("aidoc");
    p
}

/// Open the .aidoc package, inject a row that violates the identity
/// validator (empty node id), then save back.
fn build_broken_package(path: &std::path::Path) {
    let mut pkg_path = path.to_path_buf();
    let (mut package, mut store) = aidoc::open_package(&mut pkg_path).expect("open .aidoc package");
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            tx.execute(
                "INSERT INTO nodes(doc_id, id, kind, parent, position, content, attributes) \
                 VALUES(?1, '', 'section', NULL, 0, '', '{}')",
                rusqlite::params!["validate_exit"],
            )
            .map_err(anyhow::Error::from)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    aidoc::save_package(&mut package, &store).expect("save package");
}

#[test]
fn validate_exits_nonzero_on_errors() {
    let bin = binary_path();
    assert!(
        bin.exists(),
        "aidoc binary not built at {} — run `cargo build -p aidoc-cli` first",
        bin.display()
    );

    let tmp = std::env::temp_dir().join("aidoc-validate-exit-test");
    let _ = std::fs::remove_dir_all(&tmp);
    let pkg = tmp.join("broken.aidoc");

    let status = Command::new(&bin)
        .args([
            "init",
            pkg.to_str().unwrap(),
            "--doc-id",
            "validate_exit",
            "--title",
            "Validate Exit",
        ])
        .status()
        .expect("spawn aidoc init");
    assert!(status.success(), "init failed: {status:?}");

    build_broken_package(&pkg);

    let out = Command::new(&bin)
        .args(["validate", pkg.to_str().unwrap()])
        .output()
        .expect("spawn aidoc validate");
    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("[smoke] validate stdout:\n{stdout}");
    assert!(
        !out.status.success(),
        "validate should exit non-zero on a broken package, got: {:?}\nstdout: {stdout}",
        out.status
    );
    let code = out.status.code().unwrap_or(0);
    assert!((1..=125).contains(&code), "exit code out of range: {code}");
    assert!(
        stdout.contains("identity"),
        "expected identity validator to fire, got stdout: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn validate_exits_zero_on_clean_package() {
    let bin = binary_path();
    let tmp = std::env::temp_dir().join("aidoc-validate-clean-test");
    let _ = std::fs::remove_dir_all(&tmp);
    let pkg = tmp.join("clean.aidoc");

    let status = Command::new(&bin)
        .args([
            "init",
            pkg.to_str().unwrap(),
            "--doc-id",
            "validate_clean",
            "--title",
            "Clean",
        ])
        .status()
        .expect("spawn aidoc init");
    assert!(status.success());

    let out = Command::new(&bin)
        .args(["validate", pkg.to_str().unwrap()])
        .output()
        .expect("spawn aidoc validate");
    assert!(
        out.status.success(),
        "validate should exit 0 on a clean package, got: {:?}",
        out.status
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
