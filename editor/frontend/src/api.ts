import type { BoardModel, IcManifest, IcSummary } from "./types";

const BASE = "/api";

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + path, init);
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`${res.status} ${path}: ${text}`);
  }
  return res.json() as Promise<T>;
}

export function fetchLibrary(): Promise<IcSummary[]> {
  return call("/library");
}

export function fetchManifest(id: string): Promise<IcManifest> {
  return call(`/library/${id}`);
}

export async function exportBoard(model: BoardModel): Promise<string> {
  const data = await call<{ yaml: string }>("/export", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ model }),
  });
  return data.yaml;
}

export async function importBoard(yaml: string): Promise<BoardModel> {
  return call<BoardModel>("/import", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ yaml }),
  });
}

export async function validateBoard(model: BoardModel): Promise<string[]> {
  const data = await call<{ errors: string[] }>("/validate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ model }),
  });
  return data.errors;
}
