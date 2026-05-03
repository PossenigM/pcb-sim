# Firmware HAL examples

Reference implementations of the firmware-side HAL. These are **not**
part of the simulator — they are templates for whoever writes firmware
that talks to the simulator.

The HAL exposes a minimal blocking C-like API:

```
int  pcb_sim_connect(const char *socket_path);
int  pcb_sim_i2c_write(const char *bus, uint8_t addr, const uint8_t *data, size_t len);
int  pcb_sim_i2c_read (const char *bus, uint8_t addr, uint8_t *buf, size_t len);
int  pcb_sim_gpio_write(const char *pin, pcb_sim_pin_value_t value);
int  pcb_sim_gpio_read (const char *pin, pcb_sim_pin_value_t *out_value);
// ...etc.
```

Internally each call serializes a `ClientMessage` (MessagePack +
length-prefix), writes it to the Unix socket, blocks waiting for the
matching response by `id`, and returns the result. Asynchronous events
(`gpio_event`, `uart_rx`) are delivered to a user-registered callback
or queued for later polling.

## Subdirectories

- `c/` — C reference, depends on `msgpack-c`.
- `rust/` — Rust reference, depends on `rmp-serde`.

## Conformance

A HAL is "conformant" if it can drive the `examples/dev_board_v1.yaml`
board through every operation in the wire protocol spec. A conformance
test suite is TODO.
