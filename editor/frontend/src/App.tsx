import { useEffect, useState } from "react";
import LibraryPalette from "./library/LibraryPalette";
import BoardCanvas from "./canvas/BoardCanvas";

interface IcSummary {
  id: string;
  version: string;
  kind: string;
  description?: string;
  icon_url?: string;
}

export default function App() {
  const [library, setLibrary] = useState<IcSummary[]>([]);

  useEffect(() => {
    fetch("/api/library")
      .then((r) => r.json())
      .then((data) => setLibrary(data))
      .catch((err) => {
        // TODO: surface errors in the UI rather than just console
        console.error("failed to load library", err);
      });
  }, []);

  return (
    <div style={{ display: "flex", height: "100vh" }}>
      <aside style={{ width: 240, borderRight: "1px solid #ddd", overflowY: "auto" }}>
        <LibraryPalette ics={library} />
      </aside>
      <main style={{ flex: 1 }}>
        <BoardCanvas />
      </main>
    </div>
  );
}
