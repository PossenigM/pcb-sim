# ADR 0007 — Firmware runs externally; sim does not host firmware

## Context

We considered three models:

1. The simulator runs firmware (e.g., emulating an STM32 with QEMU).
2. The simulator hosts firmware as a loadable module.
3. Firmware runs as a separate Linux process and connects via IPC.

## Decision

Firmware is always an external process. The simulator does not run
firmware code. The MCU is the boundary of the simulated world; from the
simulator's perspective, the MCU is just a Unix socket plus a list of
declared interfaces and pins.

## Consequences

- The simulator has no MCU-specific code. The same simulator binary works
  for STM32, ESP32, RP2040, anything.
- Firmware is debugged with normal Linux tools (gdb, valgrind, address
  sanitizer).
- IC behaviors never need to talk to firmware directly; they talk to
  pins and buses, which the IPC adapter forwards.
- Cost: firmware authors must use a HAL that targets the wire protocol.
  Reference HALs are provided in `firmware-hal-example/`.
- The IPC wire protocol is now the most stable interface in the system
  (see ADR 0002 and the wire protocol spec).
