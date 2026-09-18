//! End-to-end smoke test for the aidoc-mcp binary.
//!
//! Spawns the compiled `aidoc-mcp` binary, exchanges the MCP JSON-RPC handshake
//! over stdio, and verifies initialize / tools/list / init_aidoc / list_nodes.
//!
//! Run with:
//!   cargo test -p aidoc-mcp --test stdio_smoke -- --nocapture

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn binary_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
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
    p.push("target");
    p.push("mcp-smoke");
    std::fs::create_dir_all(&p).expect("create tmp dir");
    p
}

async fn send_request<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    id: Option<u64>,
    method: &str,
    params: Value,
) {
    let mut body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    if let Some(id) = id {
        body["id"] = json!(id);
    }
    let body_str = serde_json::to_string(&body).expect("serialize");
    w.write_all(format!("Content-Length: {}\r\n\r\n", body_str.len()).as_bytes())
        .await
        .expect("write header");
    w.write_all(body_str.as_bytes()).await.expect("write body");
    w.flush().await.expect("flush");
}

async fn read_response<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Value {
    let mut header = String::new();
    let mut first_line = String::new();
    r.read_line(&mut first_line)
        .await
        .expect("read first line");
    eprintln!("[smoke] first line: {first_line:?}");
    if first_line.is_empty() {
        panic!("server closed connection");
    }
    header.push_str(&first_line);
    while let Ok(n) = r.read_line(&mut first_line).await {
        if n == 0 {
            break;
        }
        if first_line == "\r\n" || first_line == "\n" {
            break;
        }
        header.push_str(&first_line);
    }
    eprintln!("[smoke] full header: {header:?}");
    let len = header
        .lines()
        .find_map(|l| l.trim_start_matches('\u{feff}').strip_prefix("Content-Length: "))
        .map(|v| v.trim().parse::<usize>())
        .transpose()
        .ok()
        .flatten()
        .expect("Content-Length present");
    let mut buf = vec![0u8; len];
    tokio::io::AsyncReadExt::read_exact(r, &mut buf)
        .await
        .expect("read body");
    eprintln!("[smoke] body: {}", String::from_utf8_lossy(&buf));
    serde_json::from_slice(&buf).expect("parse json-rpc")
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
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));

    // Drain stderr in background to avoid blocking.
    tokio::spawn(async move {
        let mut lines = stderr.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[mcp stderr] {line}");
        }
    });

    // 1. initialize
    send_request(
        &mut stdin,
        Some(1),
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "stdio_smoke", "version": "0.1.0"}
        }),
    )
    .await;
    let init_resp = read_response(&mut stdout).await;
    assert_eq!(init_resp["jsonrpc"], "2.0");
    assert_eq!(init_resp["id"], 1);
    let server_info = &init_resp["result"]["serverInfo"];
    assert_eq!(server_info["name"], "aidoc-mcp");
    let protocol = init_resp["result"]["protocolVersion"]
        .as_str()
        .expect("protocol version");
    assert_eq!(protocol, "2024-11-05");

    // notifications/initialized — required by MCP after the client receives the initialize result.
    send_request(&mut stdin, None, "notifications/initialized", json!({})).await;

    // 2. tools/list
    send_request(&mut stdin, Some(2), "tools/list", json!({})).await;
    let tools_resp = read_response(&mut stdout).await;
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
    send_request(
        &mut stdin,
        Some(3),
        "tools/call",
        json!({
            "name": "init_aidoc",
            "arguments": {
                "path": pkg_path_str,
                "doc_id": "mcp_smoke",
                "title": "MCP Smoke"
            }
        }),
    )
    .await;
    let init_call = read_response(&mut stdout).await;
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
    send_request(
        &mut stdin,
        Some(4),
        "tools/call",
        json!({"name": "list_nodes", "arguments": {"path": pkg_path_str}}),
    )
    .await;
    let list_call = read_response(&mut stdout).await;
    let content = &list_call["result"]["content"];
    assert!(content.is_array(), "expected content array, got {list_call:?}");
    let text_blob = content
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text_blob.contains("root"),
        "list_nodes output missing root: {text_blob}"
    );

    drop(stdin);
    let _ = child.wait().await;
    let _ = std::fs::remove_dir_all(&pkg_path);
}