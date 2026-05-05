import { memo } from "react";
import { Handle, NodeProps, Position } from "reactflow";
import type { IcNodeData } from "../types";

const KIND_COLORS: Record<string, string> = {
  firmware_host: "#374151",
  sensor: "#1d4ed8",
  actuator: "#b91c1c",
  io_expander: "#047857",
  logic: "#7c3aed",
  other: "#6b7280",
};

const PROTOCOL_COLORS: Record<string, string> = {
  i2c: "#2563eb",
  spi: "#16a34a",
  uart: "#d97706",
};

export default memo(function IcNode({ data, selected }: NodeProps<IcNodeData>) {
  const { instance, manifest, busIfaces, netPins } = data;
  const headerColor = KIND_COLORS[manifest.kind] ?? KIND_COLORS.other;

  return (
    <div style={{
      background: "#fff",
      border: `2px solid ${selected ? "#f59e0b" : "#d1d5db"}`,
      borderRadius: 6,
      minWidth: 180,
      boxShadow: selected ? "0 0 0 2px #fde68a" : "0 1px 4px rgba(0,0,0,.15)",
      fontFamily: "system-ui, sans-serif",
      fontSize: 12,
    }}>
      {/* Header */}
      <div style={{
        background: headerColor,
        color: "#fff",
        padding: "5px 8px",
        borderRadius: "4px 4px 0 0",
        fontWeight: 700,
        fontSize: 13,
      }}>
        {instance.id}
      </div>
      <div style={{ padding: "2px 8px 1px", fontSize: 11, color: "#6b7280", borderBottom: "1px solid #e5e7eb" }}>
        {manifest.id}
      </div>

      {/* Body: interfaces left, pins right */}
      <div style={{ display: "flex", padding: "4px 0" }}>
        {/* Bus interfaces — left column */}
        <div style={{ flex: 1, paddingLeft: 0 }}>
          {busIfaces.map((iface) => (
            <div key={iface.name} style={{ position: "relative", padding: "3px 8px 3px 14px", display: "flex", alignItems: "center", gap: 4 }}>
              <Handle
                type="source"
                position={Position.Left}
                id={`iface:${iface.name}`}
                style={{ left: -6, width: 10, height: 10, background: PROTOCOL_COLORS[iface.protocol] ?? "#888", border: "none" }}
              />
              <span style={{
                display: "inline-block",
                background: PROTOCOL_COLORS[iface.protocol] ?? "#888",
                color: "#fff",
                borderRadius: 3,
                padding: "1px 4px",
                fontSize: 10,
                fontWeight: 600,
                textTransform: "uppercase",
              }}>
                {iface.protocol}
              </span>
              <span style={{ color: "#374151" }}>{iface.name}</span>
            </div>
          ))}
        </div>

        {/* Standalone pins — right column */}
        {netPins.length > 0 && (
          <div style={{ flex: 1, paddingRight: 0 }}>
            {netPins.map((pin) => (
              <div key={pin} style={{ position: "relative", padding: "3px 14px 3px 8px", display: "flex", alignItems: "center", justifyContent: "flex-end", gap: 4 }}>
                <span style={{ color: "#374151" }}>{pin}</span>
                <Handle
                  type="source"
                  position={Position.Right}
                  id={`pin:${pin}`}
                  style={{ right: -6, width: 10, height: 10, background: "#6b7280", border: "none" }}
                />
              </div>
            ))}
          </div>
        )}
      </div>

      {/* MQTT badge row */}
      {(manifest.mqtt?.publish?.length || manifest.mqtt?.subscribe?.length) && (
        <div style={{ borderTop: "1px solid #e5e7eb", padding: "3px 8px", display: "flex", gap: 4, flexWrap: "wrap" }}>
          {manifest.mqtt?.subscribe?.map((ch) => (
            <span key={ch.name} style={{ fontSize: 10, background: "#dbeafe", color: "#1e40af", borderRadius: 3, padding: "1px 4px" }}>
              ↓ {ch.name}
            </span>
          ))}
          {manifest.mqtt?.publish?.map((ch) => (
            <span key={ch.name} style={{ fontSize: 10, background: "#dcfce7", color: "#166534", borderRadius: 3, padding: "1px 4px" }}>
              ↑ {ch.name}
            </span>
          ))}
        </div>
      )}
    </div>
  );
});
