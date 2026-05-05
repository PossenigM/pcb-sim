import { useCallback, useEffect, useRef, useState } from "react";
import { ReactFlowProvider } from "reactflow";
import LibraryPalette from "./library/LibraryPalette";
import BoardCanvas, { BoardCanvasRef } from "./canvas/BoardCanvas";
import ConfigPanel from "./panel/ConfigPanel";
import { fetchLibrary, fetchManifest, exportBoard, importBoard, validateBoard } from "./api";
import type { BoardMeta, ComponentConfig, IcManifest, IcNodeData, IcSummary } from "./types";
import { flowToBoard } from "./canvas/flowConvert";

export default function App() {
  const [library, setLibrary] = useState<IcSummary[]>([]);
  const [selectedNode, setSelectedNode] = useState<IcNodeData | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [meta, setMeta] = useState<BoardMeta>({ name: "untitled" });
  const canvasRef = useRef<BoardCanvasRef | null>(null);
  const manifestCache = useRef<Record<string, IcManifest>>({});

  useEffect(() => {
    fetchLibrary()
      .then(setLibrary)
      .catch((e) => console.error("library load failed", e));
  }, []);

  // -------------------------------------------------------------------------
  // Save (export to YAML)
  // -------------------------------------------------------------------------
  const handleSave = useCallback(async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const model = flowToBoard(canvas.getNodes(), canvas.getEdges(), meta);
    try {
      const yaml = await exportBoard(model);
      const blob = new Blob([yaml], { type: "text/yaml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${meta.name || "board"}.yaml`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert(`Export failed: ${e}`);
    }
  }, [meta]);

  // -------------------------------------------------------------------------
  // Load (import from YAML file)
  // -------------------------------------------------------------------------
  const handleLoad = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".yaml,.yml";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const text = await file.text();
      try {
        const model = await importBoard(text);
        // Pre-fetch all manifests so nodes render correctly
        const manifests: Record<string, IcManifest> = {};
        await Promise.all(
          model.components.map(async (comp) => {
            const bare = comp.type.split("@")[0];
            if (!manifestCache.current[comp.type] && !manifestCache.current[bare]) {
              try {
                const m = await fetchManifest(comp.type);
                manifests[comp.type] = m;
                manifests[bare] = m;
              } catch {
                const m = await fetchManifest(bare).catch(() => null);
                if (m) { manifests[comp.type] = m; manifests[bare] = m; }
              }
            } else {
              manifests[comp.type] = manifestCache.current[comp.type] ?? manifestCache.current[bare];
              manifests[bare] = manifests[comp.type];
            }
          })
        );
        manifestCache.current = { ...manifestCache.current, ...manifests };
        setMeta({
          name: model.board.name,
          version: model.board.version,
          mqtt: model.mqtt,
        });
        canvasRef.current?.loadModel(model, manifests);
        setSelectedNode(null);
        setErrors([]);
      } catch (e) {
        alert(`Import failed: ${e}`);
      }
    };
    input.click();
  }, []);

  // -------------------------------------------------------------------------
  // Validate
  // -------------------------------------------------------------------------
  const handleValidate = useCallback(async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const model = flowToBoard(canvas.getNodes(), canvas.getEdges(), meta);
    try {
      const errs = await validateBoard(model);
      setErrors(errs);
    } catch (e) {
      setErrors([String(e)]);
    }
  }, [meta]);

  // -------------------------------------------------------------------------
  // Node config changes from ConfigPanel
  // -------------------------------------------------------------------------
  const handleNodeChange = useCallback((id: string, instance: ComponentConfig) => {
    canvasRef.current?.updateNodeData(id, instance);
    setSelectedNode((sel) =>
      sel?.instance.id === id ? { ...sel, instance } : sel
    );
  }, [canvasRef]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", fontFamily: "system-ui, sans-serif" }}>
      {/* Toolbar */}
      <header style={{
        display: "flex", alignItems: "center", gap: 10, padding: "6px 12px",
        background: "#1e293b", color: "#f1f5f9", flexShrink: 0,
      }}>
        <span style={{ fontWeight: 700, fontSize: 15, marginRight: 6 }}>pcb-sim editor</span>

        <input
          value={meta.name}
          onChange={(e) => setMeta((m) => ({ ...m, name: e.target.value }))}
          style={{ padding: "3px 6px", borderRadius: 4, border: "1px solid #475569", background: "#334155", color: "#f1f5f9", fontSize: 13, width: 180 }}
          placeholder="board name"
        />

        <div style={{ flex: 1 }} />

        {errors.length > 0 && (
          <span style={{
            background: "#dc2626", color: "#fff", borderRadius: 4,
            padding: "2px 8px", fontSize: 12, cursor: "pointer",
          }}
            onClick={() => setErrors([])}
          >
            {errors.length} error{errors.length > 1 ? "s" : ""} ✕
          </span>
        )}

        <Btn onClick={handleValidate}>Validate</Btn>
        <Btn onClick={handleLoad}>Open…</Btn>
        <Btn onClick={handleSave} primary>Save YAML</Btn>
      </header>

      {/* Validation errors */}
      {errors.length > 0 && (
        <div style={{
          background: "#fef2f2", borderBottom: "1px solid #fecaca",
          padding: "6px 12px", flexShrink: 0,
        }}>
          {errors.map((e, i) => (
            <div key={i} style={{ fontSize: 12, color: "#991b1b", fontFamily: "monospace" }}>{e}</div>
          ))}
        </div>
      )}

      {/* Main area */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Left palette */}
        <aside style={{ width: 220, borderRight: "1px solid #e5e7eb", overflowY: "auto", flexShrink: 0 }}>
          <LibraryPalette ics={library} />
        </aside>

        {/* Canvas */}
        <main style={{ flex: 1, position: "relative" }}>
          <ReactFlowProvider>
            <BoardCanvas
              canvasRef={canvasRef}
              onNodeSelect={setSelectedNode}
            />
          </ReactFlowProvider>
        </main>

        {/* Right config panel */}
        {selectedNode && (
          <aside style={{
            width: 280, borderLeft: "1px solid #e5e7eb", overflowY: "auto", flexShrink: 0,
          }}>
            <ConfigPanel
              data={selectedNode}
              onChange={(updated) => handleNodeChange(selectedNode.instance.id, updated)}
            />
          </aside>
        )}
      </div>
    </div>
  );
}

function Btn({ onClick, children, primary }: { onClick: () => void; children: React.ReactNode; primary?: boolean }) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "4px 12px",
        borderRadius: 4,
        border: "none",
        background: primary ? "#2563eb" : "#475569",
        color: "#fff",
        cursor: "pointer",
        fontSize: 13,
        fontWeight: 500,
      }}
    >
      {children}
    </button>
  );
}
