"""JSON Schema validation for board YAML and IC manifests.

Wraps `jsonschema` and adds the cross-validation rules listed in
`docs/board-yaml-spec.md` (address conflicts, pin double-claims, etc.).
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import jsonschema


def _load_schema(schema_dir: Path, name: str) -> dict[str, Any]:
    with (schema_dir / name).open() as f:
        return json.load(f)


def validate_manifest_yaml(payload: dict[str, Any], *, schema_dir: Path) -> list[str]:
    """Return a list of error messages. Empty list means valid."""
    schema = _load_schema(schema_dir, "ic-manifest.schema.json")
    return _run_jsonschema(payload, schema)


def validate_board_yaml(payload: dict[str, Any], *, schema_dir: Path) -> list[str]:
    """Return a list of error messages. Empty list means valid.

    Performs JSON Schema validation only. Cross-validation against the IC
    library happens separately — see `cross_validate_board`.
    """
    schema = _load_schema(schema_dir, "board.schema.json")
    return _run_jsonschema(payload, schema)


def _run_jsonschema(payload: dict[str, Any], schema: dict[str, Any]) -> list[str]:
    validator = jsonschema.Draft202012Validator(schema)
    errors: list[str] = []
    for err in sorted(validator.iter_errors(payload), key=lambda e: e.path):
        path = "/".join(str(p) for p in err.absolute_path) or "<root>"
        errors.append(f"{path}: {err.message}")
    return errors


def cross_validate_board(_board: dict[str, Any], _library) -> list[str]:
    """Cross-reference checks beyond what JSON Schema can express.

    See docs/board-yaml-spec.md for the full list.
    TODO: implement.
    """
    return []
