import type { Edge, Node } from "reactflow";
import type {
  BoardMeta,
  BoardModel,
  BusEdgeData,
  BusEntry,
  EdgeData,
  IcManifest,
  IcNodeData,
  NetEdgeData,
  NetEntry,
} from "../types";

const GRID_SCALE = 120; // canvas px per board layout unit

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function busIfaceNames(manifest: IcManifest): Set<string> {
  const busPins = new Set<string>();
  for (const iface of manifest.interfaces) {
    if (iface.protocol !== "gpio") {
      for (const p of iface.pins) busPins.add(p);
    }
  }
  return busPins;
}

export function deriveNodeData(manifest: IcManifest, instance: import("../types").ComponentConfig): IcNodeData {
  const busPinNames = busIfaceNames(manifest);
  const netPins = manifest.pins
    .map((p) => p.name)
    .filter((n) => !busPinNames.has(n));
  const busIfaces = manifest.interfaces.filter((i) => i.protocol !== "gpio");
  return { instance, manifest, netPins, busIfaces };
}

// ---------------------------------------------------------------------------
// Board model → React Flow
// ---------------------------------------------------------------------------

export function boardModelToFlow(
  model: BoardModel,
  manifests: Record<string, IcManifest>
): { nodes: Node<IcNodeData>[]; edges: Edge<EdgeData>[] } {
  const nodes: Node<IcNodeData>[] = model.components.map((comp) => {
    const manifest = manifests[comp.type] ?? manifests[comp.type.split("@")[0]];
    const pos = model.layout?.[comp.id] ?? { x: 0, y: 0 };
    return {
      id: comp.id,
      type: "ic",
      position: { x: pos.x * GRID_SCALE, y: pos.y * GRID_SCALE },
      data: manifest ? deriveNodeData(manifest, comp) : fallbackNodeData(comp),
    };
  });

  const edges: Edge<EdgeData>[] = [];

  for (const bus of model.buses ?? []) {
    if (bus.protocol === "i2c") {
      const members = bus.members;
      const master = members.find((m) => {
        const man = lookupManifest(nodes, m.component);
        const iface = man?.interfaces.find((i) => i.name === m.interface);
        return iface?.role === "master";
      }) ?? members[0];
      const slaves = members.filter((m) => m !== master);
      for (const slave of slaves) {
        edges.push(makeBusEdge(bus.id, bus.protocol, master, slave));
      }
    } else if (bus.protocol === "spi") {
      for (const slave of bus.slaves) {
        edges.push(makeBusEdge(bus.id, bus.protocol, bus.master, slave));
      }
    } else if (bus.protocol === "uart") {
      if (bus.peers.length >= 2) {
        edges.push(makeBusEdge(bus.id, bus.protocol, bus.peers[0], bus.peers[1]));
      }
    }
  }

  for (const net of model.nets ?? []) {
    // Star topology from first endpoint to all others.
    const [first, ...rest] = net.endpoints;
    for (const ep of rest) {
      const edgeId = `${net.id}__${first.component}.${first.pin}__${ep.component}.${ep.pin}`;
      const data: NetEdgeData = {
        kind: "net",
        netId: net.id,
        sourcePin: first.pin,
        targetPin: ep.pin,
      };
      edges.push({
        id: edgeId,
        source: first.component,
        sourceHandle: `pin:${first.pin}`,
        target: ep.component,
        targetHandle: `pin:${ep.pin}`,
        data,
        style: NET_STYLE,
        label: net.id,
      });
    }
  }

  return { nodes, edges };
}

function makeBusEdge(
  busId: string,
  protocol: BusEdgeData["protocol"],
  a: { component: string; interface: string },
  b: { component: string; interface: string }
): Edge<BusEdgeData> {
  const data: BusEdgeData = {
    kind: "bus",
    busId,
    protocol,
    sourceIface: a.interface,
    targetIface: b.interface,
  };
  return {
    id: `${busId}__${a.component}.${a.interface}__${b.component}.${b.interface}`,
    source: a.component,
    sourceHandle: `iface:${a.interface}`,
    target: b.component,
    targetHandle: `iface:${b.interface}`,
    data,
    style: busStyle(protocol),
    label: `${busId} (${protocol.toUpperCase()})`,
  };
}

function lookupManifest(
  nodes: Node<IcNodeData>[],
  compId: string
): IcManifest | undefined {
  return nodes.find((n) => n.id === compId)?.data.manifest;
}

function fallbackNodeData(comp: import("../types").ComponentConfig): IcNodeData {
  return {
    instance: comp,
    manifest: {
      id: comp.type,
      version: "?",
      kind: "other",
      interfaces: [],
      pins: [],
    },
    netPins: [],
    busIfaces: [],
  };
}

// ---------------------------------------------------------------------------
// React Flow → Board model
// ---------------------------------------------------------------------------

export function flowToBoard(
  nodes: Node<IcNodeData>[],
  edges: Edge<EdgeData>[],
  meta: BoardMeta
): BoardModel {
  const components = nodes.map((n) => n.data.instance);

  const layout: BoardModel["layout"] = {};
  for (const n of nodes) {
    layout[n.id] = {
      x: Math.round(n.position.x / GRID_SCALE),
      y: Math.round(n.position.y / GRID_SCALE),
    };
  }

  const buses = buildBuses(edges, nodes);
  const nets = buildNets(edges);

  return {
    board: { name: meta.name, version: meta.version },
    layout,
    mqtt: meta.mqtt,
    components,
    buses,
    nets,
  };
}

function buildBuses(edges: Edge<EdgeData>[], nodes: Node<IcNodeData>[]): BusEntry[] {
  // After this filter e.data is guaranteed BusEdgeData; use ! to satisfy strict null checks.
  const busEdges = edges.filter((e): e is Edge<BusEdgeData> => e.data?.kind === "bus");

  const grouped = new Map<string, Edge<BusEdgeData>[]>();
  for (const e of busEdges) {
    const d = e.data!;
    const g = grouped.get(d.busId) ?? [];
    g.push(e);
    grouped.set(d.busId, g);
  }

  const result: BusEntry[] = [];
  for (const [busId, group] of grouped) {
    const protocol = group[0].data!.protocol;

    if (protocol === "i2c") {
      const seen = new Map<string, string>(); // comp -> iface
      for (const e of group) {
        const d = e.data!;
        seen.set(e.source, d.sourceIface);
        seen.set(e.target, d.targetIface);
      }
      result.push({
        id: busId,
        protocol: "i2c",
        members: [...seen.entries()].map(([component, iface]) => ({ component, interface: iface })),
      });
    } else if (protocol === "spi") {
      let master: { component: string; interface: string } | undefined;
      const slaves: Array<{ component: string; interface: string; cs_pin_on_master: string }> = [];
      for (const e of group) {
        const d = e.data!;
        const srcIface = lookupManifest(nodes, e.source)?.interfaces.find((i) => i.name === d.sourceIface);
        if (srcIface?.role === "master") {
          master ??= { component: e.source, interface: d.sourceIface };
          slaves.push({ component: e.target, interface: d.targetIface, cs_pin_on_master: "NSS" });
        } else {
          master ??= { component: e.target, interface: d.targetIface };
          slaves.push({ component: e.source, interface: d.sourceIface, cs_pin_on_master: "NSS" });
        }
      }
      if (master) result.push({ id: busId, protocol: "spi", master, slaves });
    } else if (protocol === "uart") {
      const d = group[0].data!;
      result.push({
        id: busId,
        protocol: "uart",
        peers: [
          { component: group[0].source, interface: d.sourceIface },
          { component: group[0].target, interface: d.targetIface },
        ],
      });
    }
  }

  return result;
}

function buildNets(edges: Edge<EdgeData>[]): NetEntry[] {
  const netEdges = edges.filter((e): e is Edge<NetEdgeData> => e.data?.kind === "net");

  const grouped = new Map<string, Edge<NetEdgeData>[]>();
  for (const e of netEdges) {
    const d = e.data!;
    const g = grouped.get(d.netId) ?? [];
    g.push(e);
    grouped.set(d.netId, g);
  }

  const result: NetEntry[] = [];
  for (const [netId, group] of grouped) {
    const seen = new Set<string>();
    const endpoints: NetEntry["endpoints"] = [];
    for (const e of group) {
      const d = e.data!;
      const sk = `${e.source}:${d.sourcePin}`;
      const tk = `${e.target}:${d.targetPin}`;
      if (!seen.has(sk)) { seen.add(sk); endpoints.push({ component: e.source, pin: d.sourcePin }); }
      if (!seen.has(tk)) { seen.add(tk); endpoints.push({ component: e.target, pin: d.targetPin }); }
    }
    result.push({ id: netId, endpoints });
  }

  return result;
}

// ---------------------------------------------------------------------------
// Edge styles
// ---------------------------------------------------------------------------

const NET_STYLE = { stroke: "#666", strokeWidth: 2 } as const;

function busStyle(protocol: string) {
  const colors: Record<string, string> = {
    i2c: "#2563eb",
    spi: "#16a34a",
    uart: "#d97706",
  };
  return { stroke: colors[protocol] ?? "#888", strokeWidth: 2 };
}

// Re-export for use in canvas
export { busStyle, NET_STYLE };
