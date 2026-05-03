# ADR 0003 — Code-only IC behaviors (no declarative DSL)

## Context

For sensors with simple register layouts, a declarative register-mapping
table in the manifest could avoid Rust code per IC. For complex ICs
(GPIO expanders with state machines, sensors with calibration math) a
declarative DSL would inevitably grow until it became a bad
general-purpose language.

## Decision

All IC behaviors are Rust code, registered by name (`builtin:foo`) in the
`sim-behaviors` crate. The manifest declares which behavior to use; it
does not encode behavior logic.

## Consequences

- One escape hatch, used uniformly: simple ICs get short Rust files,
  complex ICs get longer ones, but the abstraction never bends.
- No need to design and maintain a DSL.
- Cost: adding a new IC requires writing Rust. Acceptable — we expect
  most users to consume the library, not extend it. Contributing a new
  IC behavior is a documented workflow.
- Future option: declarative shortcuts can be added later as code
  generators that emit `IcBehavior` impls. Cleaner than a runtime DSL.
