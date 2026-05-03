# Wire protocol specification

The wire protocol is the contract between firmware and the simulator.
Once stable, **changes break firmware** — version it carefully.

## 1. Transport

- **Socket type:** Unix domain socket, `SOCK_STREAM`.
- **Path:** specified per `firmware_host` component in the board YAML.
- **Connections per socket:** 1. The simulator accepts one connection at a
  time. If a second connects, the first is closed.

## 2. Framing

Every message is framed as:

```
[ 4-byte length, little-endian, unsigned ] [ MessagePack body ]
```

The length covers the body only. Maximum body size: 16 MiB. Larger
messages are a protocol error.

## 3. Encoding

- MessagePack (https://msgpack.org/).
- All maps use **string keys**.
- Byte payloads use the **`bin` family** (`bin 8`/`bin 16`/`bin 32`),
  never `str`.
- Floats must be **finite** (no NaN, no Infinity).
- Integers fit in `i64` unless otherwise noted.

## 4. Message structure

Every message is a map containing at minimum:

```
{ "type": "<string>", ... }
```

Request/response messages additionally carry an `id` (any `i64`, chosen
by the firmware, unique per in-flight request).

Events (server-pushed) do not carry `id`.

Every message MAY carry a `meta` map for forward-compatible extensions.
Receivers MUST ignore unknown `meta` keys.

## 5. Handshake

Firmware MUST send `hello` as the first message. The simulator replies
with `hello_ack` or `error`.

```
firmware → sim:
{ "type": "hello", "protocol_version": 1, "client": "<string>" }

sim → firmware:
{ "type": "hello_ack", "protocol_version": 1, "mcu_id": "<string>" }
```

If `protocol_version` is unsupported, the simulator replies with `error`
(code `unsupported_version`) and closes the connection.

## 6. Operations

### 6.1 I2C

```
i2c_write:
  request : { type, id, bus, address, data }
  response: { type: "i2c_ack", id, result: "ok" | "nack" }

i2c_read:
  request : { type, id, bus, address, length }
  response: { type: "i2c_data", id, result: "ok" | "nack", data? }

i2c_write_read:
  request : { type, id, bus, address, write, read_length }
  response: { type: "i2c_data", id, result: "ok" | "nack", data? }
```

`bus` is the bus id from the board YAML. `data`/`write` are `bin`. On
NACK, `data` is omitted.

### 6.2 SPI

```
spi_transfer:
  request : { type, id, bus, cs, mosi }
  response: { type: "spi_data", id, result: "ok", miso }
```

`cs` is the CS pin name on the master. `miso` length always equals
`mosi` length.

### 6.3 GPIO

```
gpio_write:
  request : { type, id, pin, value }     # value: "low" | "high" | "z"
  response: { type: "gpio_ack", id, result: "ok" }

gpio_read:
  request : { type, id, pin }
  response: { type: "gpio_value", id, result: "ok", value }

gpio_configure:
  request : { type, id, pin, direction, pull? }
                    # direction: "in" | "out"
                    # pull: "none" | "up" | "down"  (optional)
  response: { type: "gpio_ack", id, result: "ok" }
```

### 6.4 UART

UART is async on both sides; no `id`.

```
uart_tx (firmware → sim):
  { type, bus, data }

uart_rx (sim → firmware):
  { type, bus, data, sim_time_us }
```

## 7. Events (sim → firmware, unsolicited)

```
gpio_event:
  { type, pin, value, sim_time_us }

uart_overflow_event:
  { type, bus, dropped_bytes, sim_time_us }

simulator_event:
  { type, event: "shutdown" | "reset" | "warning",
    message?, sim_time_us }
```

`sim_time_us` is monotonic microseconds since simulator start.

## 8. Errors

```
{ type: "error", id?, code, message }
```

`id` is present iff the error is in response to a specific request.
Codes (extensible):

- `unsupported_version`
- `no_such_bus`
- `no_such_pin`
- `bad_request`
- `internal_error`

## 9. Versioning policy

- `protocol_version` starts at 1.
- **Non-breaking:** adding new message types, adding new optional fields,
  adding new error codes.
- **Breaking:** changing field types, removing fields, changing
  request/response correlation rules. Bumps `protocol_version`.
- The simulator MAY support multiple versions; firmware sends the version
  it speaks in `hello`.

## 10. Concurrency

- Firmware MAY have multiple in-flight requests; `id` correlates them.
- The simulator MAY interleave responses with events at any time.
- The simulator processes requests serially internally (single-threaded
  event loop), but firmware should not assume FIFO response ordering —
  always match by `id`.

## 11. Reference implementations

See `firmware-hal-example/c/` and `firmware-hal-example/rust/`.
