export interface IcSummary {
  id: string;
  version: string;
  kind: string;
  description?: string;
  icon_url?: string;
}

export interface IcPinSpec {
  name: string;
  dir: "in" | "out" | "bidir";
  default?: string;
}

export interface IcIfaceConfigField {
  type: string;
  default?: unknown;
  choices?: unknown[];
  range?: [unknown, unknown];
}

export interface IcInterface {
  name: string;
  protocol: "i2c" | "spi" | "uart" | "gpio";
  role?: "master" | "slave" | "peer";
  pins: string[];
  config?: Record<string, IcIfaceConfigField>;
}

export interface IcMqttChannel {
  name: string;
  type: string;
  unit?: string;
}

export interface IcManifest {
  id: string;
  version: string;
  kind: string;
  description?: string;
  interfaces: IcInterface[];
  pins: IcPinSpec[];
  mqtt?: {
    subscribe?: IcMqttChannel[];
    publish?: IcMqttChannel[];
  };
  behavior?: string;
}

export interface ComponentConfig {
  id: string;
  type: string;
  config?: Record<string, unknown>;
  initial_values?: Record<string, unknown>;
  mqtt?: {
    publish?: Record<string, string>;
    subscribe?: Record<string, string>;
  };
  firmware?: { transport: "unix_socket"; path: string };
}

export interface BoardMeta {
  name: string;
  version?: string;
  mqtt?: { broker?: string; prefix?: string };
}

export interface BoardModel {
  board: { name: string; version?: string };
  layout: Record<string, { x: number; y: number }>;
  mqtt?: { broker?: string; prefix?: string };
  components: ComponentConfig[];
  buses: BusEntry[];
  nets: NetEntry[];
}

export type BusEntry =
  | { id: string; protocol: "i2c"; members: Array<{ component: string; interface: string }> }
  | { id: string; protocol: "spi"; master: { component: string; interface: string }; slaves: Array<{ component: string; interface: string; cs_pin_on_master: string }> }
  | { id: string; protocol: "uart"; peers: Array<{ component: string; interface: string }> };

export interface NetEntry {
  id: string;
  endpoints: Array<{ component: string; pin: string }>;
}

// Edge data stored on React Flow edges
export type BusEdgeData = {
  kind: "bus";
  busId: string;
  protocol: "i2c" | "spi" | "uart";
  sourceIface: string;
  targetIface: string;
  sourceRole?: string;
  targetRole?: string;
};

export type NetEdgeData = {
  kind: "net";
  netId: string;
  sourcePin: string;
  targetPin: string;
};

export type EdgeData = BusEdgeData | NetEdgeData;

// Node data stored in React Flow nodes
export interface IcNodeData {
  instance: ComponentConfig;
  manifest: IcManifest;
  /** Pins available for net connections = pins not in any i2c/spi/uart interface */
  netPins: string[];
  /** Interfaces available for bus connections */
  busIfaces: IcInterface[];
}
