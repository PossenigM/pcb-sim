# ADR 0002 — MessagePack for IPC

## Context

The firmware ↔ simulator wire protocol carries small request/response
messages with binary payloads (I2C/SPI data buffers). Options considered:

- JSON / NDJSON
- MessagePack
- Protobuf / FlatBuffers
- Custom binary format

## Decision

MessagePack with a 4-byte little-endian length prefix per message.

## Consequences

- Native binary support: no base64 overhead for I2C/SPI buffers.
- Schema-less: messages are self-describing maps; we document shapes in
  prose rather than maintaining an IDL.
- Library support in C, Rust, Python, and most embedded toolchains.
- Cost: less human-readable than JSON. Mitigated by a debug logging mode
  that pretty-prints messages.
- Future option: if we ever want strict schema enforcement at the wire
  level, layer something like JSON Schema or Cap'n Proto on top. Not
  needed for v1.
