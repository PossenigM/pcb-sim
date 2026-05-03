"""Editor model ↔ board YAML serialization.

The editor's internal model is a JSON-serializable representation that
mirrors the board YAML schema with editor-only additions (component
positions, UI hints). This module converts between the two.
"""

from __future__ import annotations

from typing import Any


def editor_model_to_board_yaml(_model: dict[str, Any]) -> str:
    """Serialize the editor's model to a YAML string."""
    # TODO: produce a clean, human-readable YAML matching examples/.
    raise NotImplementedError


def board_yaml_to_editor_model(_yaml_str: str) -> dict[str, Any]:
    """Parse a board YAML and return the editor's internal model."""
    # TODO: parse, then layer in any editor-only defaults (e.g. assign
    # x/y positions for components missing layout entries).
    raise NotImplementedError
