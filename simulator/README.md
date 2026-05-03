# Simulator (Rust)

Cargo workspace containing the `pcb-sim` simulator.

## Crates

| Crate           | Responsibility                                              |
|-----------------|-------------------------------------------------------------|
| `sim-core`      | Trait definitions, event loop, bus and net routers.         |
| `sim-protocol`  | MessagePack wire types for the firmware IPC protocol.       |
| `sim-config`    | YAML loaders, JSON Schema validation, derived types.        |
| `sim-behaviors` | Built-in IC behavior implementations and registry.          |
| `sim-mqtt`      | MQTT client adapter, topic ↔ channel routing.               |
| `sim-ipc`       | Unix socket adapter for firmware connections.               |
| `sim-bin`       | The binary; wires everything together.                      |

## Building

```bash
cargo build
cargo test
```

## Running

```bash
cargo run --bin sim-bin -- \
    --board ../examples/minimal_blink.yaml \
    --library ../ic-library
```

## Status

Scaffolding. See each crate's `lib.rs` for TODOs.

## Development workflow

1. Pick the smallest end-to-end slice you can run (probably
   `minimal_blink.yaml` against `sim-core` + `sim-config` + `sim-ipc`).
2. Use the Python `tools/manual_firmware_client.py` helper to send raw
   IPC messages while developing.
3. Add MQTT only after the IPC path works end-to-end.
