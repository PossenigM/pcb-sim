"""Editor model ↔ board YAML serialization.

The editor's internal model is the board YAML structure (a plain dict)
with the optional ``layout`` section always present (auto-generated for
components that lack explicit positions).  This module converts between
the model dict and a YAML string.
"""

from __future__ import annotations

import io
import math
from typing import Any

import yaml


# Keys written in this order in the output YAML.
_TOP_LEVEL_ORDER = ("board", "layout", "mqtt", "components", "buses", "nets")

# Grid spacing (arbitrary canvas units) for auto-layout.
_GRID_SPACING = 4


def editor_model_to_board_yaml(model: dict[str, Any]) -> str:
    """Serialize the editor model to a human-readable YAML string."""
    ordered: dict[str, Any] = {}
    for key in _TOP_LEVEL_ORDER:
        if key in model:
            ordered[key] = model[key]
    for key, val in model.items():
        if key not in ordered:
            ordered[key] = val

    stream = io.StringIO()
    yaml.dump(
        ordered,
        stream,
        default_flow_style=False,
        allow_unicode=True,
        sort_keys=False,
        indent=2,
    )
    return stream.getvalue()


def board_yaml_to_editor_model(yaml_str: str) -> dict[str, Any]:
    """Parse board YAML and return the editor model.

    Guarantees that every component has a ``layout`` entry so the canvas
    can place it immediately without checking for missing keys.
    """
    board = yaml.safe_load(yaml_str)
    if not isinstance(board, dict):
        raise ValueError("board YAML must be a mapping at the top level")

    _ensure_layout(board)
    return board


def _ensure_layout(board: dict[str, Any]) -> None:
    """Add grid-positioned layout entries for any component without one."""
    components = board.get("components") or []
    layout = board.setdefault("layout", {})

    # Only auto-place components that are entirely absent from layout.
    unplaced = [c["id"] for c in components if isinstance(c, dict) and c.get("id") and c["id"] not in layout]
    if not unplaced:
        return

    cols = max(1, math.ceil(math.sqrt(len(components))))
    # Start auto-placed components after the rightmost/bottommost existing entry.
    start_x = max((v.get("x", 0) for v in layout.values()), default=0) + _GRID_SPACING
    for idx, comp_id in enumerate(unplaced):
        row, col = divmod(idx, cols)
        layout[comp_id] = {
            "x": start_x + col * _GRID_SPACING,
            "y": 1 + row * _GRID_SPACING,
        }
