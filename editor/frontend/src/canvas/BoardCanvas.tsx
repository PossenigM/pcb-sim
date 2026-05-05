import { useCallback, useRef } from "react";
import ReactFlow, {
  addEdge,
  Background,
  Connection,
  ConnectionMode,
  Controls,
  Edge,
  MiniMap,
  Node,
  NodeTypes,
  useEdgesState,
  useNodesState,
  useReactFlow,
} from "reactflow";
import type { EdgeData, IcManifest, IcNodeData, ComponentConfig } from "../types";
import { deriveNodeData, boardModelToFlow, busStyle, NET_STYLE } from "./flowConvert";
import { fetchManifest } from "../api";
import IcNode from "./IcNode";
import type { BusEdgeData, NetEdgeData } from "../types";

const NODE_TYPES: NodeTypes = { ic: IcNode };

let _idSeq = 1;
function nextId(prefix: string) { return `${prefix}${_idSeq++}`; }

export interface BoardCanvasRef {
  getNodes: () => Node<IcNodeData>[];
  getEdges: () => Edge<EdgeData>[];
  loadModel: (model: import("../types").BoardModel, manifests: Record<string, IcManifest>) => void;
  updateNodeData: (id: string, instance: ComponentConfig) => void;
}

interface Props {
  canvasRef: React.MutableRefObject<BoardCanvasRef | null>;
  onNodeSelect: (data: IcNodeData | null) => void;
}

export default function BoardCanvas({ canvasRef, onNodeSelect }: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<IcNodeData>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<EdgeData>([]);
  const { screenToFlowPosition } = useReactFlow();
  const manifestCache = useRef<Record<string, IcManifest>>({});

  // Expose imperative API to parent
  canvasRef.current = {
    getNodes: () => nodes,
    getEdges: () => edges,
    loadModel(model, manifests) {
      manifestCache.current = { ...manifestCache.current, ...manifests };
      const { nodes: n, edges: e } = boardModelToFlow(model, manifests);
      setNodes(n);
      setEdges(e);
    },
    updateNodeData(id, instance) {
      setNodes((nds) =>
        nds.map((n) =>
          n.id === id ? { ...n, data: { ...n.data, instance } } : n
        )
      );
    },
  };

  // -------------------------------------------------------------------------
  // Drop IC from palette
  // -------------------------------------------------------------------------
  const onDragOver = useCallback((evt: React.DragEvent) => {
    evt.preventDefault();
    evt.dataTransfer.dropEffect = "copy";
  }, []);

  const onDrop = useCallback(async (evt: React.DragEvent) => {
    evt.preventDefault();
    const icId = evt.dataTransfer.getData("application/ic_id");
    if (!icId) return;

    let manifest = manifestCache.current[icId];
    if (!manifest) {
      try { manifest = await fetchManifest(icId); }
      catch { return; }
      manifestCache.current[icId] = manifest;
    }

    const version = manifest.version;
    const compType = `${icId}@${version}`;
    const compId = nextId(icId.split("/").pop()! + "_");
    const position = screenToFlowPosition({ x: evt.clientX, y: evt.clientY });

    const instance: ComponentConfig = { id: compId, type: compType };
    if (manifest.kind === "firmware_host") {
      instance.firmware = { transport: "unix_socket", path: `/tmp/board_sim/${compId}.sock` };
    }

    const newNode: Node<IcNodeData> = {
      id: compId,
      type: "ic",
      position,
      data: deriveNodeData(manifest, instance),
    };
    setNodes((nds) => [...nds, newNode]);
  }, [screenToFlowPosition, setNodes]);

  // -------------------------------------------------------------------------
  // Connect handles
  // -------------------------------------------------------------------------
  const onConnect = useCallback((connection: Connection) => {
    const { source, target, sourceHandle, targetHandle } = connection;
    if (!source || !target || !sourceHandle || !targetHandle) return;

    const srcIsIface = sourceHandle.startsWith("iface:");
    const tgtIsIface = targetHandle.startsWith("iface:");

    if (srcIsIface && tgtIsIface) {
      // Bus connection
      const srcIface = sourceHandle.replace("iface:", "");
      const tgtIface = targetHandle.replace("iface:", "");

      // Determine protocol from source node's manifest
      const srcNode = nodes.find((n) => n.id === source);
      const srcManifest = srcNode?.data.manifest;
      const iface = srcManifest?.interfaces.find((i) => i.name === srcIface);
      const protocol = (iface?.protocol ?? "i2c") as BusEdgeData["protocol"];

      const busId = nextId("bus_");
      const data: BusEdgeData = {
        kind: "bus",
        busId,
        protocol,
        sourceIface: srcIface,
        targetIface: tgtIface,
        sourceRole: iface?.role,
      };
      const edge: Edge<BusEdgeData> = {
        id: `${busId}__${source}.${srcIface}__${target}.${tgtIface}`,
        source,
        sourceHandle,
        target,
        targetHandle,
        data,
        style: busStyle(protocol),
        label: `${busId} (${protocol.toUpperCase()})`,
      };
      setEdges((eds) => addEdge(edge, eds));
    } else if (!srcIsIface && !tgtIsIface) {
      // Net connection
      const srcPin = sourceHandle.replace("pin:", "");
      const tgtPin = targetHandle.replace("pin:", "");
      const netId = nextId("net_");
      const data: NetEdgeData = { kind: "net", netId, sourcePin: srcPin, targetPin: tgtPin };
      const edge: Edge<NetEdgeData> = {
        id: `${netId}__${source}.${srcPin}__${target}.${tgtPin}`,
        source,
        sourceHandle,
        target,
        targetHandle,
        data,
        style: NET_STYLE,
        label: netId,
      };
      setEdges((eds) => addEdge(edge, eds));
    }
    // Mismatched (iface↔pin) connections are silently ignored.
  }, [nodes, setEdges]);

  // -------------------------------------------------------------------------
  // Node selection → config panel
  // -------------------------------------------------------------------------
  const onNodeClick = useCallback((_: React.MouseEvent, node: Node<IcNodeData>) => {
    onNodeSelect(node.data);
  }, [onNodeSelect]);

  const onPaneClick = useCallback(() => {
    onNodeSelect(null);
  }, [onNodeSelect]);

  // -------------------------------------------------------------------------
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={NODE_TYPES}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onNodeClick={onNodeClick}
      onPaneClick={onPaneClick}
      connectionMode={ConnectionMode.Loose}
      snapToGrid
      snapGrid={[20, 20]}
      fitView
      deleteKeyCode="Delete"
    >
      <Background gap={20} color="#e5e7eb" />
      <MiniMap
        nodeColor={(n: Node<IcNodeData>) => {
          const colors: Record<string, string> = {
            firmware_host: "#374151", sensor: "#1d4ed8",
            actuator: "#b91c1c", io_expander: "#047857",
          };
          return colors[n.data?.manifest?.kind] ?? "#9ca3af";
        }}
      />
      <Controls />
    </ReactFlow>
  );
}
