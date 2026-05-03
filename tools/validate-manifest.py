#!/usr/bin/env python3
"""Validate one or more IC manifests against the JSON Schema.

Usage:
    python tools/validate-manifest.py path/to/manifest.yaml [more.yaml ...]

Exits 0 if every file is valid, 1 otherwise.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import yaml

try:
    import jsonschema
except ImportError:
    sys.stderr.write("error: jsonschema not installed. Run: pip install jsonschema pyyaml\n")
    sys.exit(2)

REPO_ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = REPO_ROOT / "schema" / "ic-manifest.schema.json"


def main() -> int:
    if len(sys.argv) < 2:
        sys.stderr.write(f"usage: {sys.argv[0]} <manifest.yaml> [more...]\n")
        return 2

    with SCHEMA_PATH.open() as f:
        schema = json.load(f)
    validator = jsonschema.Draft202012Validator(schema)

    overall_ok = True
    for path_str in sys.argv[1:]:
        path = Path(path_str)
        try:
            with path.open() as f:
                data = yaml.safe_load(f)
        except (OSError, yaml.YAMLError) as e:
            print(f"{path}: FAILED to parse: {e}", file=sys.stderr)
            overall_ok = False
            continue

        errors = list(validator.iter_errors(data))
        if errors:
            overall_ok = False
            print(f"{path}: INVALID")
            for err in errors:
                location = "/".join(str(p) for p in err.absolute_path) or "<root>"
                print(f"    {location}: {err.message}")
        else:
            print(f"{path}: ok")

    return 0 if overall_ok else 1


if __name__ == "__main__":
    sys.exit(main())
