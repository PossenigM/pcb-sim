# Architecture

This document captures the design of `pcb-sim`. It is the single
authoritative reference for how the components fit together. When in doubt,
this document wins; if reality disagrees with this document, fix one of them.

## 1. Goals and non-goals

### Goals

- Simulate the **logical** behavior of a PCB: digital interfaces (I2C, SPI,
  UART, GPIO) and the ICs connected by them.
- Run an external **firmware process** unmodified (or close to it) against
  the simulated board, via a thin Hardware Abstraction Layer (HAL).
- Bridge **sensors and actuators** to an external physics engine via MQTT,
  so the simulation can be closed-loop (firmware affects environment,
  environment affects sensors).
- Provide a **board definition format** (YAML) that captures only what the
  simulator needs — interfaces, ICs, connections, MQTT topics. Power
  delivery, decoupling, mechanical layout: out of scope.
- Provide a **web-based editor** to author board definitions visually.
- Maintain a **reusable IC library** so common components (sensors, IO
  expanders, etc.) are written once and shared across boards.

### Non-goals

- Physics simulation. A separate engine (already exists) handles that and
  communicates over MQTT.
- Electrical simulation: voltages, currents, signal integrity, timing
  analysis.
- Cycle-accurate MCU emulation. The MCU is the boundary of the simulated
  world; firmware runs as a normal Linux process.
- Multi-master I2C arbitration (in v1).
- Simulating the MCU's own peripheral logic. Firmware sees a HAL; the
  simulator does not model the STM32's I2C peripheral registers.

## 2. High-level architecture

```
┌──────────────────────┐         ┌──────────────────┐
│  Editor Web-App      │ writes  │  Board YAML      │
│  (Python + React)    │────────▶│  (board defn)    │
│  - grid placement    │         └────────┬─────────┘
│  - IO connections    │                  │ reads
│  - validates against │                  ▼
│    IC library        │         ┌──────────────────┐
└──────────┬───────────┘  reads  │  Simulator       │
           │                     │  (Rust)          │
           ▼                     │                  │
   ┌──────────────────┐  reads   │  - bus router    │
   │  IC Library      │◀─────────│  - net router    │
   │  (manifests +    │          │  - IC behaviors  │
   │   icons)         │          │  - virtual clock │
   └──────────────────┘          │  - IPC adapter   │
                                 └──┬────────┬──────┘
                                    │        │
                            ┌───────┘        └────────┐
                            ▼                         ▼
                   ┌─────────────────┐      ┌──────────────────┐
                   │  Firmware       │      │  MQTT broker     │
                   │  (your process) │      │  ↕ physics engine│
                   └─────────────────┘      └──────────────────┘
```

There is exactly **one firmware process** in v1, connecting to one MCU
boundary in the simulator. The simulator does not run firmware itself.

## 3. Core concepts

### 3.1 IC library

A directory of reusable IC definitions. Each IC has:

- A **manifest** (`manifest.yaml`) describing its pins, interfaces,
  configurable parameters, and MQTT publish/subscribe channels.
- An **icon** (`icon.svg`) for the editor.
- A **behavior** — Rust code in `simulator/crates/sim-behaviors/`
  implementing the `IcBehavior` trait, looked up at runtime by the
  manifest's `behavior: builtin:foo` field.

The library is data, not code. The editor consumes manifests directly. The
simulator consumes manifests + maps `behavior:` to a registered Rust impl.

### 3.2 Component kinds

Every IC is one of:

- **`firmware_host`** — the MCU. Has no behavior code; the simulator wires
  an IPC adapter to its declared interfaces and pins. Firmware connects
  over a Unix socket.
- **`peripheral`** — anything else. Has a behavior. Subkinds for editor
  metadata: `sensor`, `actuator`, `io_expander`, `logic`, etc.

### 3.3 Buses and nets

Two distinct routing layers:

- **Buses** carry protocol-level transactions (I2C, SPI, UART). They
  connect components via **interfaces** (named groups of pins). Each bus
  protocol has a per-protocol schema reflecting its real topology
  (I2C: flat membership + addressing; SPI: master + slaves with CS pins;
  UART: two peers).
- **Nets** carry single-wire digital signals (GPIO connections, interrupt
  lines). They connect components via **pins** directly.

A pin used as part of a bus interface is owned by the bus router. A pin
used in a net belongs to the net router. A pin cannot be in both.

### 3.4 MQTT bridging

The MQTT direction is from the **IC's** point of view:

- **Sensors subscribe.** Their physical input (temperature, light, etc.)
  comes from the physics engine via MQTT. The sensor's behavior caches the
  latest value and presents it via its bus interface when firmware reads.
- **Actuators publish.** When firmware drives a pin or sends data, the
  actuator's behavior publishes the resulting state to MQTT.

Topic strings are bound at the **board YAML** level — manifests declare
channel names (`temperature`), boards map them to topics
(`sensors/room1/temperature`).

If an MQTT subscription has no value yet, the sensor returns a configurable
**initial value** specified in the board YAML. The simulator never blocks
firmware on missing MQTT data.

### 3.5 Behavior model

Each peripheral implements the `IcBehavior` trait
(see `simulator/crates/sim-core/src/trait_def.rs`):

```rust
pub trait IcBehavior: Send {
    fn init(&mut self, ctx: &mut InitCtx) -> Result<(), IcError>;

    fn on_pin_change(&mut self, pin: PinId, value: PinValue,
                     ctx: &mut RunCtx) -> Result<(), IcError> { Ok(()) }

    fn on_bus_transaction(&mut self, txn: BusTransaction<'_>,
                          ctx: &mut RunCtx) -> Result<BusResponse, IcError>
        { Err(IcError::Unsupported("bus transactions")) }

    fn on_mqtt_message(&mut self, channel: &str, payload: MqttValue,
                       ctx: &mut RunCtx) -> Result<(), IcError> { Ok(()) }

    fn on_tick(&mut self, now: SimTime,
               ctx: &mut RunCtx) -> Result<(), IcError> { Ok(()) }
}
```

Outputs (pin writes, MQTT publishes, bus master transactions, scheduled
ticks) flow through `RunCtx`, never as direct calls. This keeps behaviors
testable and side-effect-free at the type level.

### 3.6 Wire protocol (firmware ↔ simulator)

- **Transport:** Unix socket (one per `firmware_host` component).
- **Encoding:** MessagePack with a 4-byte little-endian length prefix.
- **Versioning:** every connection begins with a `hello` exchange that
  negotiates `protocol_version`.
- **Operations:** I2C (write/read/write-read), SPI (transfer), UART
  (tx/rx), GPIO (write/read/configure), GPIO change events, errors.
- **Semantics:** request/response is correlated by `id`; events
  (`gpio_event`, `uart_rx`) are pushed by the simulator without IDs.
- **Time:** events carry `sim_time_us` so firmware can reason about
  ordering and latency.

See [wire-protocol.md](wire-protocol.md) for the full specification.

## 4. Simulator internals

### 4.1 Event loop

Single-threaded priority queue ordered by virtual `SimTime`. Events come
from four sources:

1. **IPC adapter** — firmware sent a request.
2. **MQTT subscriber** — broker delivered a message.
3. **Tick scheduler** — an IC asked to be ticked at a specific time.
4. **Net cascade** — one IC's pin write triggered another IC's
   `on_pin_change`.

I/O happens on separate async tasks (Tokio) that *enqueue* into the main
loop. Behaviors and routers run synchronously inside the loop, which is
why `IcBehavior` is sync — no GIL-equivalent gymnastics, no locking.

### 4.2 I2C router

- Owns a bus identity and an address → slave lookup table.
- On transaction: look up the addressed slave; dispatch to its
  `on_bus_transaction`; return its `BusResponse`.
- No address match → `BusResponse::Nack`.
- Address conflicts in board YAML are detected at **load time** and the
  simulator refuses to start.
- v1 supports a single master per bus (the IPC adapter / MCU).

### 4.3 SPI router

- Slaves are selected by CS pin, not by address.
- The board YAML must declare each slave's CS pin on the master.
- Full-duplex: MOSI bytes in, MISO bytes out (same length).

### 4.4 UART router

- Point-to-point, two peers per UART bus.
- Buffered both directions (default 64KB per direction).
- When the buffer fills, drop incoming bytes and emit a warning event.

### 4.5 Net router

Permissive resolution with prominent warnings:

- All endpoints `HighZ` → net is `HighZ`.
- One non-Z driver → net takes that value.
- Multiple drivers, same value → net takes that value.
- Multiple drivers, different values → **contention**: log a prominent
  warning, resolve to LOW (arbitrary but consistent), continue.

When a net's resolved value changes, all endpoints with the pin declared
`dir: in` or `dir: bidir` receive `on_pin_change`.

## 5. Editor

- **Backend:** FastAPI. Loads the IC library, validates board YAML
  against JSON Schema, exports/imports YAML. Stateless.
- **Frontend:** React + React Flow. Library palette, drag-and-drop onto a
  canvas, draw connections between pin handles, configure per-component
  parameters and MQTT topic mappings.
- **Validation:** real-time, against the same JSON Schema the simulator
  uses. Single source of truth.

## 6. Build and deploy

- The simulator is a single Rust binary (`sim-bin`).
- The editor is a Python ASGI app (FastAPI) plus a static-built React
  bundle.
- The IC library and schemas are language-neutral data files, consumed
  by both.
- A monorepo, because the schema is shared and splitting it forces
  premature versioning ceremony.

## 7. Open questions

These are deferred decisions. When you encounter them in implementation,
revisit and add an ADR.

- **Firmware reconnection semantics.** When firmware disconnects mid-run,
  do peripherals reset, retain state, or block reconnection? Tentative
  default: retain state (option 2 in the design discussion).
- **GPIO bank shorthand syntax.** Writing 16 PA pins by hand is painful.
  Add a `range:` or pattern expansion to the manifest schema.
- **Plugin loading.** Builtin behaviors are sufficient for v1. A future
  version may load behaviors from `cdylib` or WASM.
- **Per-channel MQTT QoS / retained settings.** Currently everything
  defaults to QoS 0 with retained subscriptions for sensors. May need
  per-channel overrides.

## 8. Decision history

See [adr/](adr/) for the rationale behind major decisions.
