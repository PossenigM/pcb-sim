# pcb-sim

A logical (non-physical) PCB simulator for embedded systems development.
Simulates digital interfaces and signals — I2C, SPI, UART, GPIO — between an
external firmware process and a configurable set of simulated ICs (sensors,
actuators, IO expanders, etc.). Sensors and actuators bridge to a physics
engine via MQTT.

This repository contains:

- **Schema and IC library** — the data formats and component catalog.
- **Simulator** (Rust) — the runtime that loads a board YAML and simulates it.
- **Editor** (Python + React) — a web app to author board YAML files visually.
- **Firmware HAL examples** — reference HAL implementations in C and Rust.

## Quick links

- [Architecture overview](docs/architecture.md)
- [IC manifest specification](docs/ic-manifest-spec.md)
- [Board YAML specification](docs/board-yaml-spec.md)
- [Wire protocol specification](docs/wire-protocol.md)
- [Architecture Decision Records](docs/adr/)

## Status

Pre-alpha. Scaffolding only. See individual `README.md` files in each
subdirectory for the current state of each component.

## Repository layout

```
pcb-sim/
├── docs/                  # Architecture and specification documents
├── schema/                # JSON Schemas for IC manifests and board YAML
├── ic-library/            # Reusable IC catalog (manifests + icons)
├── examples/              # Sample board definitions
├── simulator/             # Rust workspace — the simulator itself
├── editor/                # Python + React web app for authoring boards
├── firmware-hal-example/  # Reference firmware HAL implementations
└── tools/                 # CLI utilities for validation
```

## Getting started

Build order recommended for first-time setup:

1. Read `docs/architecture.md`.
2. Validate the example board against the schema:
   `python tools/validate-board.py examples/dev_board_v1.yaml`
3. Build the simulator: `cd simulator && cargo build`
4. Run the editor backend: `cd editor && uvicorn backend.app:app --reload`

See each subdirectory's README for detailed instructions.

## License

TBD.
