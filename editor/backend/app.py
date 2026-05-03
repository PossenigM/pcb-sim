"""FastAPI app for the pcb-sim editor.

The backend is intentionally thin: it loads the IC library, validates
board YAML against the JSON Schema, and serializes/deserializes the
editor's internal representation.

Run:
    uvicorn backend.app:app --reload
"""

from __future__ import annotations

import os
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware

from .library import IcLibrary
from .validation import validate_board_yaml, validate_manifest_yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LIBRARY = REPO_ROOT / "ic-library"
DEFAULT_SCHEMA_DIR = REPO_ROOT / "schema"

LIBRARY_PATH = Path(os.environ.get("PCB_SIM_LIBRARY", DEFAULT_LIBRARY))
SCHEMA_DIR = Path(os.environ.get("PCB_SIM_SCHEMA_DIR", DEFAULT_SCHEMA_DIR))

app = FastAPI(title="pcb-sim editor backend", version="0.1.0")

# Permissive CORS for local development. Tighten for production deploys.
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# Loaded once at startup. Reload by restarting the server.
library: IcLibrary | None = None


@app.on_event("startup")
def _load_library() -> None:
    global library
    # TODO: handle errors gracefully (a single bad manifest should not
    # prevent the editor from starting; surface the error in the UI).
    library = IcLibrary.load(LIBRARY_PATH)


@app.get("/api/library")
def list_library():
    """List all ICs in the library with summary metadata."""
    if library is None:
        raise HTTPException(503, "library not loaded")
    return [entry.summary() for entry in library.entries.values()]


@app.get("/api/library/{ic_id:path}")
def get_ic(ic_id: str):
    """Return the full manifest for a specific IC."""
    if library is None:
        raise HTTPException(503, "library not loaded")
    entry = library.entries.get(ic_id)
    if entry is None:
        raise HTTPException(404, f"unknown IC: {ic_id}")
    return entry.manifest


@app.post("/api/validate")
def validate(payload: dict):
    """Validate a board YAML payload. Returns a list of errors (empty on success)."""
    # TODO: decide payload shape. Probably {"yaml": "..."} or a parsed object.
    errors = validate_board_yaml(payload, schema_dir=SCHEMA_DIR)
    return {"errors": errors}


@app.post("/api/export")
def export_yaml(_payload: dict):
    """Convert the editor's internal model to a board YAML string."""
    # TODO: define the editor's internal model (likely matches the board
    # YAML closely with editor-only fields like component positions).
    raise HTTPException(501, "not yet implemented")


@app.post("/api/import")
def import_yaml(_payload: dict):
    """Parse a YAML string and return the editor's internal model."""
    # TODO
    raise HTTPException(501, "not yet implemented")


@app.get("/health")
def health():
    return {"status": "ok", "library_loaded": library is not None}
