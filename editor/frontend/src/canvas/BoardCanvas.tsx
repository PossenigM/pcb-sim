import { ReactFlow, Background, Controls, MiniMap } from "reactflow";

/**
 * The board editor canvas.
 *
 * TODO:
 *   - Custom node types (one per IC kind) rendering pin handles.
 *   - Drop handler: when a library item is dropped, instantiate a node.
 *   - onConnect: validate that source and target pins belong to compatible
 *     interfaces (same protocol, complementary roles).
 *   - Per-component config side panel (I2C address, MQTT topic mapping).
 *   - Snap-to-grid (gridSize=20 or similar).
 *   - Save/load via /api/export and /api/import.
 *   - Undo/redo.
 */
export default function BoardCanvas() {
  return (
    <ReactFlow nodes={[]} edges={[]} fitView>
      <Background gap={20} />
      <MiniMap />
      <Controls />
    </ReactFlow>
  );
}
