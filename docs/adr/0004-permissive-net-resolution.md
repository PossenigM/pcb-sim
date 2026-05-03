# ADR 0004 — Permissive net resolution

## Context

When multiple IC pins drive the same net to different values, the
simulator must decide what value the net takes and whether to halt.

## Decision

Permissive resolution with prominent warnings:

- All endpoints HighZ → net is HighZ.
- One non-Z driver → net takes that value.
- Multiple drivers, same value → net takes that value.
- Multiple drivers, different values → log a prominent warning, resolve
  to LOW (arbitrary but consistent), continue.

## Consequences

- Firmware development scenarios that exercise edge cases don't crash
  the simulator.
- Bugs are visible (warnings) but not blocking.
- A future "strict mode" CLI flag could escalate contention to a halt
  for CI runs where any contention is a regression.
- Cost: noisy logs are possible if a bug genuinely produces sustained
  contention. Mitigated by deduplicating warnings per net.
