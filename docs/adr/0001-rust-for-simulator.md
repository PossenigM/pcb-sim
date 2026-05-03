# ADR 0001 — Rust for the simulator

## Context

The simulator is a long-running event-driven process with many concurrent
"actors" (ICs), real-time-ish I/O (MQTT, IPC sockets), and the need to
remain stable for hours during firmware development sessions. We
considered Python, Go, and Rust.

## Decision

The simulator is implemented in Rust.

## Consequences

- Strong typing catches schema/behavior mismatches at compile time.
- No GIL, so the event loop and async I/O can scale without contortions.
- Memory safety reduces crash surface for an always-on tool.
- The IC behavior trait can rely on zero-cost abstractions
  (`Box<dyn IcBehavior>` dispatch is fine).
- Cost: Rust learning curve for contributors. Mitigated by keeping the
  `IcBehavior` trait small and the per-IC code mechanical.
- The editor remains in Python — different tradeoffs (rapid UI iteration,
  rich ecosystem for web), and no shared runtime needed since the schema
  is the contract.
