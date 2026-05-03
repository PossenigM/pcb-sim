# IC manifest specification

An IC manifest describes a reusable component in the IC library. Each IC has
its own directory under `ic-library/` containing a `manifest.yaml` and an
`icon.svg`. Manifests are language-neutral data; the simulator's behavior
code lives separately and is matched by the manifest's `behavior:` field.

## Top-level structure

```yaml
id: vendor/part_name           # required, unique
version: 0.1                   # required, semver-ish
kind: sensor                   # required, see "Kinds" below
description: Free-form text    # optional but recommended

interfaces: [...]              # required, may be empty
pins: [...]                    # required
config: {...}                  # optional
mqtt:                          # optional
  publish: [...]
  subscribe: [...]
behavior: builtin:foo          # required for peripherals; absent for firmware_host
```

## Kinds

- `firmware_host` — an MCU/MPU. Has no behavior. The simulator wires an
  IPC adapter to its declared interfaces and pins. Firmware connects via
  Unix socket.
- `sensor` — produces values consumed by firmware. Typically declares
  `mqtt.subscribe` (the physics engine publishes the sensed quantity).
- `actuator` — driven by firmware, affects the world. Typically declares
  `mqtt.publish`.
- `io_expander` — bridges a bus to additional pins (e.g. MCP23017).
- `logic` — combinatorial / sequential logic (gates, flip-flops, muxes).
- Other free-form kinds are allowed for editor metadata; the simulator
  treats anything that is not `firmware_host` as a peripheral.

## Interfaces

An interface groups pins by protocol. A pin appears in **at most one**
interface.

```yaml
interfaces:
  - name: i2c                  # local-to-this-IC name
    protocol: i2c              # i2c | spi | uart | gpio
    role: slave                # slave | master | peer (uart)
    pins: [SCL, SDA]           # references to pin names declared below
    config:                    # interface-specific configurable fields
      address:
        type: hex
        default: 0x76
        choices: [0x76, 0x77]  # optional
```

Per-protocol notes:

- **i2c** roles: `master` or `slave`. Slaves typically declare an
  `address` config field.
- **spi** roles: `master` or `slave`. Slaves are selected by CS pins
  (declared in the board YAML, not the manifest).
- **uart** role: `peer`. Both ends of a UART are peers in this model.
- **gpio** has no role; it's just a group of digital pins.

## Pins

```yaml
pins:
  - name: SCL                  # required, unique within this IC
    dir: bidir                 # in | out | bidir
    default: Z                 # LOW | HIGH | Z (high-impedance)
    runtime_configurable: true # optional; default false
```

`runtime_configurable: true` indicates that firmware (or the IC's
behavior) can change the effective direction at runtime. Examples:
MCP23017 GPIO pins controlled by IODIR registers.

## Config

Per-IC configuration parameters not associated with a single interface.

```yaml
config:
  reset_delay_ms:
    type: int
    default: 5
```

Supported `type` values: `int`, `float`, `string`, `bool`, `hex`, `enum`
(with `choices:`).

## MQTT

Channels declare what the IC consumes/produces logically. The board YAML
maps channel names to topic strings.

```yaml
mqtt:
  subscribe:                   # IC consumes external values
    - name: temperature
      type: float              # bool | int | float | string | bytes
      unit: celsius            # optional, informational
  publish:                     # IC produces values
    - name: state
      type: bool
```

## Behavior

For peripherals:

```yaml
behavior: builtin:bme280
```

The string after `builtin:` is matched against the simulator's behavior
registry (`sim-behaviors` crate). For `firmware_host` ICs, omit the
`behavior` field entirely.

## Validation rules

The simulator and editor validate manifests at load time. A manifest is
**invalid** if:

- `id` is missing or not in `vendor/part` form.
- A pin is referenced by an interface but not declared in `pins`.
- A pin is referenced by more than one interface.
- A `firmware_host` declares a `behavior`.
- A non-`firmware_host` IC omits `behavior`.
- An MQTT channel `name` collides within the same direction.

See `schema/ic-manifest.schema.json` for the formal rules.

## Examples

See:

- `ic-library/st_stm32f4/manifest.yaml` — firmware_host
- `ic-library/bosch_bme280/manifest.yaml` — sensor
- `ic-library/microchip_mcp23017/manifest.yaml` — io_expander
- `ic-library/generic_gpio_led/manifest.yaml` — actuator
