import type { IcSummary } from "../types";

interface Props {
  ics: IcSummary[];
}

const KIND_ORDER = ["firmware_host", "sensor", "actuator", "io_expander", "logic", "other"];
const KIND_LABELS: Record<string, string> = {
  firmware_host: "MCU",
  sensor: "Sensors",
  actuator: "Actuators",
  io_expander: "IO Expanders",
  logic: "Logic",
  other: "Other",
};

export default function LibraryPalette({ ics }: Props) {
  const grouped = groupByKind(ics);

  return (
    <div style={{ padding: 10, fontFamily: "system-ui, sans-serif", fontSize: 13 }}>
      <div style={{ fontWeight: 700, fontSize: 14, marginBottom: 8, color: "#111827" }}>
        IC Library
      </div>
      {ics.length === 0 && <p style={{ color: "#9ca3af", fontSize: 12 }}>Loading…</p>}

      {KIND_ORDER.filter((k) => grouped[k]?.length).map((kind) => (
        <div key={kind} style={{ marginBottom: 10 }}>
          <div style={{
            fontSize: 10, fontWeight: 700, textTransform: "uppercase",
            color: "#6b7280", letterSpacing: "0.05em", marginBottom: 4,
          }}>
            {KIND_LABELS[kind] ?? kind}
          </div>

          {grouped[kind].map((ic) => (
            <div
              key={ic.id + ic.version}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData("application/ic_id", ic.id);
                e.dataTransfer.effectAllowed = "copy";
              }}
              style={{
                padding: "6px 8px",
                marginBottom: 3,
                borderRadius: 5,
                border: "1px solid #e5e7eb",
                background: "#f9fafb",
                cursor: "grab",
                userSelect: "none",
              }}
              onMouseEnter={(e) => { (e.currentTarget as HTMLDivElement).style.background = "#eff6ff"; }}
              onMouseLeave={(e) => { (e.currentTarget as HTMLDivElement).style.background = "#f9fafb"; }}
            >
              <div style={{ fontWeight: 600, color: "#111827" }}>{ic.id}</div>
              {ic.description && (
                <div style={{ fontSize: 11, color: "#6b7280", marginTop: 1, lineHeight: 1.3 }}>
                  {ic.description.slice(0, 60)}{ic.description.length > 60 ? "…" : ""}
                </div>
              )}
              <div style={{ fontSize: 10, color: "#9ca3af", marginTop: 2 }}>v{ic.version}</div>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function groupByKind(ics: IcSummary[]): Record<string, IcSummary[]> {
  const out: Record<string, IcSummary[]> = {};
  for (const ic of ics) {
    const k = ic.kind ?? "other";
    (out[k] ??= []).push(ic);
  }
  return out;
}
