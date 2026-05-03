# Board YAML specification

A board YAML file describes a specific instantiation: which ICs are on the
board, how they are connected, what addresses and topics they use. The
editor generates these files; the simulator consumes them.

## Top-level structure

```yaml
board:
  name: dev_board_v1
  version: 0.1

layout:           # editor-only metadata, simulator ignores
  mcu1: { x: 2, y: 2 }
  ...

mqtt:             # board-level MQTT config
  broker: tcp://localhost:1883
  prefix: lab1/board1/

components: [...] # required, the IC instances on this board
buses: [...]      # required, bus topologies
nets: [...]       # required, point-to-point GPIO connections (may be empty)
```

## Components

Each component is an instance of an IC from the library.

```yaml
components:
  - id: temp1                          # required, unique on this board
    type: bosch/bme280@0.1             # required, library reference
    config:                            # optional, overrides defaults
      i2c.address: 0x76
    initial_values:                    # optional, for sensors
      temperature: 20.0
      humidity: 50.0
      pressure: 1013.0
    mqtt:                              # required if manifest declares mqtt
      subscribe:
        temperature: sensors/room1/temperature
        humidity:    sensors/room1/humidity
        pressure:    sensors/room1/pressure
    firmware:                          # required if kind == firmware_host
      transport: unix_socket
      path: /tmp/board_sim/mcu1.sock
```

### `type`

`vendor/part@version` form, matching an IC manifest in the library.

### `config`

Overrides defaults from the manifest. Interface-scoped fields use
`interface_name.field` notation: `i2c.address: 0x20`.

### `initial_values`

For sensors with MQTT subscriptions: the value returned before any MQTT
message arrives. Required for every subscribed channel that the firmware
might read before the physics engine has produced a value.

### `mqtt`

Maps manifest-declared channel names to full topic strings. The board's
`mqtt.prefix` is prepended to every topic when the simulator subscribes
or publishes.

### `firmware`

Only for `kind: firmware_host` components. Declares the IPC endpoint:

- `transport: unix_socket` — the only supported transport in v1.
- `path: /tmp/...` — absolute path to the socket.

## Buses

Each bus has a per-protocol schema. The discriminator is `protocol:`.

### I2C bus

```yaml
- id: i2c_main
  protocol: i2c
  members:
    - { component: mcu1,   interface: i2c1 }   # the master
    - { component: temp1,  interface: i2c }
    - { component: ioexp1, interface: i2c }
```

In v1, exactly one member must have `role: master` in its IC manifest.

### SPI bus

```yaml
- id: spi_a
  protocol: spi
  master:
    component: mcu1
    interface: spi1
  slaves:
    - component: flash1
      interface: spi
      cs_pin_on_master: SPI1_NSS
    - component: adc1
      interface: spi
      cs_pin_on_master: PA4
```

The master's CS pin per slave must be a pin declared in the master's
GPIO interface (or the SPI interface itself).

### UART bus

```yaml
- id: uart_console
  protocol: uart
  peers:
    - { component: mcu1,   interface: uart2 }
    - { component: modem1, interface: uart }
```

Exactly two peers per UART bus.

## Nets

Point-to-point digital signal connections, for pins not part of any bus.

```yaml
nets:
  - id: net_led1
    endpoints:
      - { component: ioexp1, pin: GPA0 }
      - { component: led1,   pin: A }

  - id: net_irq
    endpoints:
      - { component: ioexp1, pin: INTA }
      - { component: mcu1,   pin: PA1 }
```

A pin used as part of a bus interface **must not** appear in any net.
The validator enforces this.

## Validation rules

A board YAML is invalid if:

- Any `type` reference cannot be resolved in the IC library.
- Any I2C bus has two slaves with the same address.
- Any SPI bus has two slaves sharing a CS pin.
- Any pin is used in both a bus interface and a net.
- A `firmware_host` lacks a `firmware:` block.
- A peripheral with `mqtt.subscribe` channels lacks corresponding entries
  in `initial_values`.
- A component's MQTT mapping references a channel name not in the
  manifest.

See `schema/board.schema.json` for the formal rules.

## Example

See `examples/dev_board_v1.yaml`.
