"""IC library loader.

Walks the ic-library directory tree, loads every manifest.yaml, and
indexes by both `vendor/part@version` (the `type:` field in board YAML)
and bare `vendor/part` (for API lookups that omit the version).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


@dataclass
class IcEntry:
    """One IC manifest plus filesystem metadata."""
    manifest: dict[str, Any]
    manifest_path: Path
    icon_path: Path | None = None

    def summary(self) -> dict[str, Any]:
        """Lightweight representation for the library listing endpoint."""
        m = self.manifest
        ic_id = m.get("id")
        return {
            "id": ic_id,
            "version": m.get("version"),
            "kind": m.get("kind"),
            "description": m.get("description"),
            "icon_url": f"/api/icons/{ic_id}" if self.icon_path else None,
        }


@dataclass
class IcLibrary:
    """All ICs known to the editor.

    Keyed two ways:
      - ``entries_versioned``: ``vendor/part@version`` — matches the
        ``type:`` field in board YAML (e.g. ``bosch/bme280@0.1``).
      - ``entries``: ``vendor/part`` — for API lookups that omit the
        version (e.g. ``GET /api/library/bosch/bme280``).

    When multiple versions of the same IC exist, ``entries`` points to
    the one with the highest version string (lexicographic, sufficient
    for semver-like version strings in v1).
    """
    entries: dict[str, IcEntry] = field(default_factory=dict)
    entries_versioned: dict[str, IcEntry] = field(default_factory=dict)

    def get(self, key: str) -> IcEntry | None:
        """Look up by bare id or versioned id."""
        return self.entries.get(key) or self.entries_versioned.get(key)

    @classmethod
    def load(cls, root: Path) -> "IcLibrary":
        lib = cls()
        if not root.exists():
            return lib

        for manifest_path in sorted(root.glob("*/manifest.yaml")):
            try:
                with manifest_path.open() as f:
                    manifest = yaml.safe_load(f)
            except Exception:
                continue

            ic_id = manifest.get("id")
            version = manifest.get("version")
            if not ic_id or not version:
                continue

            versioned_key = f"{ic_id}@{version}"
            icon_path = manifest_path.parent / "icon.svg"
            entry = IcEntry(
                manifest=manifest,
                manifest_path=manifest_path,
                icon_path=icon_path if icon_path.exists() else None,
            )
            lib.entries_versioned[versioned_key] = entry

            # Bare-id index: keep highest version (lexicographic).
            existing = lib.entries.get(ic_id)
            if existing is None or version > existing.manifest.get("version", ""):
                lib.entries[ic_id] = entry

        return lib
