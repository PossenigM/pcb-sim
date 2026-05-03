# Editor

Web-based authoring tool for board YAML files.

- **Backend:** FastAPI. Loads the IC library, validates board YAML, exports
  and imports YAML.
- **Frontend:** React + React Flow. Drag-and-drop canvas with pin handles
  and protocol-aware connection validation.

## Backend

```bash
cd backend
python -m venv .venv
source .venv/bin/activate
pip install -e ..[dev]
uvicorn app:app --reload
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

```bash
cd frontend
npm install
npm run dev
```

Vite + React + TypeScript. React Flow for the canvas. shadcn/ui (or
similar) for chrome.

## Status

Scaffolding. See backend/app.py and frontend/src/App.tsx for TODOs.
