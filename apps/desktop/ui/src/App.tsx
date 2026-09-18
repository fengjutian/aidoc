import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { NodeEditor } from "./NodeEditor";

interface Info {
  doc_id: string;
  title: string;
  head_revision: string;
  entry: string;
}

interface NodeRow {
  id: string;
  kind: string;
  parent: string | null;
  position: number;
  content: string;
}

interface RevisionRow {
  id: string;
  parent: string | null;
  operation: string;
  created_at: string;
  message: string | null;
}

export default function App() {
  const [info, setInfo] = useState<Info | null>(null);
  const [nodes, setNodes] = useState<NodeRow[]>([]);
  const [revs, setRevs] = useState<RevisionRow[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [initPath, setInitPath] = useState("");
  const [title, setTitle] = useState("");

  const refresh = async () => {
    if (!info) return;
    try {
      const [n, r] = await Promise.all([
        invoke<NodeRow[]>("list_nodes"),
        invoke<RevisionRow[]>("list_revisions"),
      ]);
      setNodes(n);
      setRevs(r);
      setInfo((prev) => (prev ? { ...prev, head_revision: r.at(-1)?.id ?? prev.head_revision } : prev));
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
  }, [info?.doc_id]);

  const onInit = async () => {
    setError(null);
    try {
      const i = await invoke<Info>("init_doc", {
        path: initPath,
        docId: title.toLowerCase().replace(/\s+/g, "-") || "demo",
        title: title || "Untitled",
      });
      setInfo(i);
    } catch (e) {
      setError(String(e));
    }
  };

  const onOpen = async () => {
    setError(null);
    try {
      const i = await invoke<Info>("open_doc", { path: initPath });
      setInfo(i);
    } catch (e) {
      setError(String(e));
    }
  };

  const onUpdate = async (target: string, html: string) => {
    try {
      await invoke<string>("update_node", { target, content: html });
    } catch (e) {
      setError(String(e));
    }
  };

  const onRevert = async (revId: string) => {
    setError(null);
    try {
      await invoke<string>("revert", { target: revId });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onSave = async () => {
    setError(null);
    try {
      await invoke("save_doc");
    } catch (e) {
      setError(String(e));
    }
  };

  const onExportHtml = async () => {
    setError(null);
    try {
      const html = await invoke<string>("export_html");
      // Open the exported HTML in a new tab.
      const blob = new Blob([html], { type: "text/html" });
      const url = URL.createObjectURL(blob);
      window.open(url, "_blank");
    } catch (e) {
      setError(String(e));
    }
  };

  if (!info) {
    return (
      <div className="topbar" style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
        <h1>AIDoc Desktop</h1>
        <p style={{ margin: 0, opacity: 0.7 }}>
          No document open. Initialize a new <code>.aidoc</code> or open an existing one.
        </p>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <input
            placeholder="examples/demo.aidoc"
            value={initPath}
            onChange={(e) => setInitPath(e.target.value)}
            style={{ flex: 1, padding: "0.3rem 0.5rem" }}
          />
          <input
            placeholder="Document title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            style={{ flex: 1, padding: "0.3rem 0.5rem" }}
          />
          <button onClick={onInit} disabled={!initPath}>
            init
          </button>
          <button onClick={onOpen} disabled={!initPath}>
            open
          </button>
        </div>
        {error && <div className="error">{error}</div>}
      </div>
    );
  }

  const active = nodes.find((n) => n.id === activeId);

  return (
    <>
      <div className="topbar">
        <h1>
          {info.title} <span style={{ opacity: 0.5 }}>· head={info.head_revision}</span>
        </h1>
        <button onClick={onSave}>save</button>
        <button onClick={onExportHtml}>export html</button>
      </div>
      {error && <div className="error">{error}</div>}
      <div className="main">
        <aside className="tree">
          <h3>Nodes</h3>
          <ul>
            {nodes
              .slice()
              .sort((a, b) => a.position - b.position)
              .map((n) => (
                <li
                  key={n.id}
                  className={n.id === activeId ? "active" : ""}
                  onClick={() => setActiveId(n.id)}
                >
                  {n.id}
                  <span className="kind">{n.kind}</span>
                </li>
              ))}
          </ul>
          <div className="history">
            <strong>History</strong>
            <ol style={{ paddingLeft: "1.2rem" }}>
              {revs.map((r) => (
                <li key={r.id}>
                  <code>{r.id}</code> {r.message ?? ""}{" "}
                  <button
                    style={{ fontSize: "0.7rem", padding: "0 0.3rem", marginLeft: "0.3rem" }}
                    onClick={() => onRevert(r.id)}
                    disabled={r.id === info.head_revision}
                  >
                    revert
                  </button>
                </li>
              ))}
            </ol>
          </div>
        </aside>
        <main className="editor">
          <div className="pane">
            {active ? (
              <NodeEditor
                key={active.id}
                kind={active.kind as never}
                content={active.content}
                onChange={(html) => onUpdate(active.id, html)}
              />
            ) : (
              <p style={{ opacity: 0.6 }}>Select a node from the tree to edit.</p>
            )}
          </div>
        </main>
      </div>
    </>
  );
}