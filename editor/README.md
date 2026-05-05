# Editor

Web-based authoring tool for board YAML files.

- **Backend:** FastAPI. Loads the IC library, validates board YAML, exports
  and imports YAML.
- **Frontend:** React + React Flow. Drag-and-drop canvas with pin handles
  and protocol-aware connection validation.

## Backend

```bash
cd ..
python -m venv .venv
source .venv/bin/activate
pip install -e .
uvicorn backend.app:app --reload
```

`.[dev]` is optional. It installs test and lint tooling and is not needed just
to run the editor:

```bash
pip install -e '.[dev]'
```

API reference (skeleton):

| Method | Path                 | Purpose                                     |
|--------|----------------------|---------------------------------------------|
| GET    | `/api/library`       | List all IC manifests with icon URLs.       |
| GET    | `/api/library/{id}`  | Full manifest for a specific IC.            |
| POST   | `/api/validate`      | Validate a board YAML (returns errors).     |
| POST   | `/api/export`        | Editor model → YAML.                        |
| POST   | `/api/import`        | YAML → editor model.                        |

## Frontend

Requires Node 18+ because this app uses Vite 5.

```bash
cd frontend
npm install
npm run dev
```

Vite + React + TypeScript. React Flow for the canvas. shadcn/ui (or
similar) for chrome.

## Status

Scaffolding. See backend/app.py and frontend/src/App.tsx for TODOs.
