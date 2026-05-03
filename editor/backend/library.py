"""IC library loader.

Walks the ic-library directory tree, loads every manifest.yaml, and
indexes by `vendor/part@version` (the form board YAML uses in `type:`).
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
        return {
            "id": m.get("id"),
            "version": m.get("version"),
            "kind": m.get("kind"),
            "description": m.get("description"),
            "icon_url": f"/api/library/{m.get('id')}/icon" if self.icon_path else None,
        }


@dataclass
class IcLibrary:
    """All ICs known to the editor, keyed by `vendor/part@version`."""
    entries: dict[str, IcEntry] = field(default_factory=dict)

    @classmethod
    def load(cls, root: Path) -> "IcLibrary":
        lib = cls()
        if not root.exists():
            # TODO: log a warning rather than silently returning empty.
            return lib

        for manifest_path in root.glob("*/manifest.yaml"):
            with manifest_path.open() as f:
                manifest = yaml.safe_load(f)
            ic_id = manifest.get("id")
            version = manifest.get("version")
            if not ic_id or not version:
                # TODO: collect these issues and surface them.
                continue
            key = f"{ic_id}@{version}"
            icon_path = manifest_path.parent / "icon.svg"
            lib.entries[key] = IcEntry(
                manifest=manifest,
                manifest_path=manifest_path,
                icon_path=icon_path if icon_path.exists() else None,
            )
        return lib
