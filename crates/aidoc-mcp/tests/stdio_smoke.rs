//! End-to-end smoke test for the aidoc-mcp binary.
//!
//! Spawns the compiled `aidoc-mcp` binary, exchanges the MCP JSON-RPC handshake
//! over stdio using rmcp 0.3.2's line-delimited JSON wire format, and
//! verifies:
//!   1. `initialize` round-trip succeeds and the server reports its name.
//!   2. `tools/list` returns the expected tool names.
//!   3. `tools/call init_aidoc` creates a real .aidoc package on disk.
//!   4. `tools/call list_nodes` reports the seed root node.
//!
//! Run with: `cargo test -p aidoc-mcp --test stdio_smoke -- --nocapture`

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn binary_path() -> PathBuf {
    // cargo runs tests from CARGO_MANIFEST_DIR (crates/aidoc-mcp). Walk up
    // to the repo root to find target/.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // -> crates/
    p.pop(); // -> repo root
    p.push("target");
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    p.push(profile);
    #[cfg(windows)]
    p.push("aidoc-mcp.exe");
    #[cfg(not(windows))]
    p.push("aidoc-mcp");
    p
}

fn workspace_tmp_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target");
    p.push("mcp-smoke");
    std::fs::create_dir_all(&p).expect("create tmp dir");
    p
}

/// Write one newline-delimited JSON-RPC message to the server's stdin.
async fn send<W: AsyncWriteExt + Unpin>(w: &mut W, msg: Value) {
    let s = serde_json::to_string(&msg).expect("serialize");
    w.write_all(s.as_bytes()).await.expect("write");
    w.write_all(b"\n").await.expect("newline");
    w.flush().await.expect("flush");
}

/// Read one newline-delimited JSON-RPC message from the server's stdout.
async fn recv<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Value {
    let mut line = String::new();
    let n = r.read_line(&mut line).await.expect("read line");
    if n == 0 {
        panic!("server closed connection");
    }
    serde_json::from_str(line.trim_end()).expect("parse json-rpc line")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_handshake_and_init_aidoc() {
    let bin = binary_path();
    assert!(
        bin.exists(),
        "aidoc-mcp binary not built at {} — run `cargo build -p aidoc-mcp` first",
        bin.display()
    );

    let tmp = workspace_tmp_dir();
    let pkg_path = tmp.join("mcp-smoke-doc.aidoc");
    let _ = std::fs::remove_dir_all(&pkg_path);

    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn aidoc-mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let stderr = child.stderr.take().expect("stderr");
    tokio::spawn(async move {
        let mut r = BufReader::new(stderr);
        let mut s = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut r, &mut s).await;
        if !s.trim().is_empty() {
            eprintln!("[mcp stderr] {s}");
        }
    });

    // 1. initialize
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "stdio_smoke", "version": "0.1.0"}
            }
        }),
    )
    .await;
    let init_resp = recv(&mut stdout).await;
    assert_eq!(init_resp["jsonrpc"], "2.0");
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "aidoc-mcp");
    assert_eq!(
        init_resp["result"]["protocolVersion"], "2024-11-05",
        "server must speak 2024-11-05"
    );

    // notifications/initialized — required after initialize response
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
    )
    .await;

    // 2. tools/list
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    )
    .await;
    let tools_resp = recv(&mut stdout).await;
    let names: Vec<&str> = tools_resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    for required in [
        "init_aidoc",
        "open_aidoc",
        "save_aidoc",
        "list_nodes",
        "show_node",
        "create_node",
        "update_node",
        "delete_node",
        "apply_operation",
        "history",
        "revert",
        "export_html",
        "validate",
    ] {
        assert!(
            names.contains(&required),
            "missing tool {required} (have {names:?})"
        );
    }

    // 3. tools/call init_aidoc
    let pkg_path_str = pkg_path.to_string_lossy().to_string();
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "init_aidoc",
                "arguments": {
                    "path": pkg_path_str,
                    "doc_id": "mcp_smoke",
                    "title": "MCP Smoke"
                }
            }
        }),
    )
    .await;
    let init_call = recv(&mut stdout).await;
    eprintln!("[smoke] init_aidoc response: {init_call:?}");
    assert!(
        init_call["error"].is_null(),
        "init_aidoc errored: {init_call:?}"
    );
    assert!(
        pkg_path.exists(),
        "init_aidoc did not create {}",
        pkg_path.display()
    );

    // 4. tools/call list_nodes
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {"name": "list_nodes", "arguments": {"path": pkg_path_str}}
        }),
    )
    .await;
    let list_call = recv(&mut stdout).await;
    let content = list_call["result"]["content"]
        .as_array()
        .expect("content array");
    let blob = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        blob.contains("root"),
        "list_nodes output missing root: {blob}"
    );

    drop(stdin);
    let _ = child.wait().await;
    let _ = std::fs::remove_dir_all(&pkg_path);
}