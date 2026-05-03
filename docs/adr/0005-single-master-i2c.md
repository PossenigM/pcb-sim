# ADR 0005 — Single-master I2C in v1

## Context

Real I2C buses can have multiple masters with arbitration. Modeling this
correctly requires simulating bus contention at a level the rest of our
abstraction doesn't reach.

## Decision

In v1, every I2C bus has exactly one master. The validator rejects board
YAMLs with more than one I2C master per bus.

## Consequences

- Router code stays simple: master initiates, slave responds, done.
- Covers the overwhelming majority of embedded designs (one MCU, many
  slaves).
- Cost: scenarios involving two cooperating masters can't be simulated
  yet. Acceptable; revisit if a real use case appears.
- Future: multi-master support is a contained change in the I2C router
  if we add it.
