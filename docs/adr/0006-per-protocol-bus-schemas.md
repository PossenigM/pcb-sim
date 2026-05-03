# ADR 0006 — Per-protocol bus schemas

## Context

I2C, SPI, and UART have genuinely different topologies (flat addressed
membership; master with CS-selected slaves; two peers). A unified
"members" schema with optional fields obscures this and forces validation
logic to second-guess the topology.

## Decision

Each bus protocol has its own schema in the board YAML, discriminated by
the `protocol:` field. JSON Schema uses `oneOf` to dispatch.

- I2C: flat `members:` list with role from manifest.
- SPI: `master:` + `slaves:` with explicit `cs_pin_on_master`.
- UART: `peers:` (exactly two).

## Consequences

- Schema reflects reality; validation errors are specific.
- Editor UI can render protocol-appropriate connection forms.
- Cost: more schema code. Acceptable — these are stable, well-known
  topologies.
