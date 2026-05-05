"""FastAPI app for the pcb-sim editor.

The backend is intentionally thin: it loads the IC library, validates
board YAML against the JSON Schema, and serializes/deserializes the
editor's internal representation.

Run:
    uvicorn backend.app:app --reload
"""

from __future__ import annotations

import os
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import Response
from pydantic import BaseModel

from .export import board_yaml_to_editor_model, editor_model_to_board_yaml
from .library import IcLibrary
from .validation import cross_validate_board, validate_board_yaml, validate_manifest_yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LIBRARY = REPO_ROOT / "ic-library"
DEFAULT_SCHEMA_DIR = REPO_ROOT / "schema"

LIBRARY_PATH = Path(os.environ.get("PCB_SIM_LIBRARY", DEFAULT_LIBRARY))
SCHEMA_DIR = Path(os.environ.get("PCB_SIM_SCHEMA_DIR", DEFAULT_SCHEMA_DIR))

# Loaded once at startup. Reload by restarting the server.
library: IcLibrary | None = None


@asynccontextmanager
async def _lifespan(_app: FastAPI):
    global library
    library = IcLibrary.load(LIBRARY_PATH)
    yield


app = FastAPI(title="pcb-sim editor backend", version="0.1.0", lifespan=_lifespan)

# Permissive CORS for local development. Tighten for production deploys.
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


# ---------------------------------------------------------------------------
# Request / response models
# ---------------------------------------------------------------------------

class ValidateRequest(BaseModel):
    """Board model dict to validate (the same structure as board YAML, parsed)."""
    model: dict[str, Any]

class ExportRequest(BaseModel):
    """Editor model → YAML string."""
    model: dict[str, Any]

class ImportRequest(BaseModel):
    """Raw YAML string → editor model."""
    yaml: str


# ---------------------------------------------------------------------------
# IC library endpoints
# ---------------------------------------------------------------------------

@app.get("/api/library")
def list_library():
    """List all ICs in the library with summary metadata."""
    _require_library()
    return [entry.summary() for entry in library.entries.values()]  # type: ignore[union-attr]


@app.get("/api/library/{ic_id:path}")
def get_ic(ic_id: str):
    """Return the full manifest for a specific IC (bare or versioned id)."""
    _require_library()
    entry = library.get(ic_id)  # type: ignore[union-attr]
    if entry is None:
        raise HTTPException(404, f"unknown IC: {ic_id}")
    return entry.manifest


@app.get("/api/icons/{ic_id:path}")
def get_icon(ic_id: str):
    """Serve the SVG icon for an IC."""
    _require_library()
    entry = library.get(ic_id)  # type: ignore[union-attr]
    if entry is None:
        raise HTTPException(404, f"unknown IC: {ic_id}")
    if entry.icon_path is None:
        raise HTTPException(404, f"no icon for IC: {ic_id}")
    return Response(content=entry.icon_path.read_bytes(), media_type="image/svg+xml")


# ---------------------------------------------------------------------------
# Board authoring endpoints
# ---------------------------------------------------------------------------

@app.post("/api/validate")
def validate(req: ValidateRequest):
    """Validate a board model. Returns a list of errors (empty on success)."""
    errors = validate_board_yaml(req.model, schema_dir=SCHEMA_DIR)
    errors += cross_validate_board(req.model, library)
    return {"errors": errors}


@app.post("/api/export")
def export_yaml(req: ExportRequest):
    """Convert the editor's internal model to a board YAML string."""
    try:
        yaml_str = editor_model_to_board_yaml(req.model)
    except Exception as exc:
        raise HTTPException(422, str(exc)) from exc
    return {"yaml": yaml_str}


@app.post("/api/import")
def import_yaml(req: ImportRequest):
    """Parse a YAML string and return the editor's internal model."""
    try:
        model = board_yaml_to_editor_model(req.yaml)
    except Exception as exc:
        raise HTTPException(422, str(exc)) from exc
    return model


# ---------------------------------------------------------------------------
# Misc
# ---------------------------------------------------------------------------

@app.get("/health")
def health():
    return {"status": "ok", "library_loaded": library is not None}


def _require_library() -> None:
    if library is None:
        raise HTTPException(503, "library not loaded")
