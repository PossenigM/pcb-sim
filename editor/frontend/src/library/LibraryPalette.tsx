interface IcSummary {
  id: string;
  version: string;
  kind: string;
  description?: string;
  icon_url?: string;
}

interface Props {
  ics: IcSummary[];
}

/**
 * Sidebar palette listing every IC in the library. Drag an entry onto
 * the canvas to instantiate it.
 *
 * TODO:
 *   - Drag handler that sets reactflow drag data.
 *   - Group by `kind` (sensors, actuators, expanders, MCUs).
 *   - Search/filter input.
 *   - Tooltips with manifest details.
 */
export default function LibraryPalette({ ics }: Props) {
  return (
    <div style={{ padding: 12 }}>
      <h3 style={{ marginTop: 0 }}>IC library</h3>
      {ics.length === 0 && <p style={{ color: "#888" }}>No ICs loaded.</p>}
      <ul style={{ listStyle: "none", padding: 0 }}>
        {ics.map((ic) => (
          <li
            key={ic.id + ic.version}
            style={{
              padding: "8px 4px",
              borderBottom: "1px solid #eee",
              cursor: "grab",
            }}
            // TODO: onDragStart handler for react-flow drop integration
          >
            <div style={{ fontWeight: 600 }}>{ic.id}</div>
            <div style={{ fontSize: 12, color: "#666" }}>
              {ic.kind} · v{ic.version}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
