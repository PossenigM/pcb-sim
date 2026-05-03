# IC library

Reusable IC definitions consumed by both the editor and the simulator.
Each IC has its own directory with:

- `manifest.yaml` — pins, interfaces, configurable parameters, MQTT
  channels. Schema: `schema/ic-manifest.schema.json`.
- `icon.svg` — used by the editor.

The simulator's runtime behavior code lives separately in
`simulator/crates/sim-behaviors/`, matched to a manifest by its
`behavior:` field (e.g. `behavior: builtin:bme280` looks up `bme280` in
the behaviors registry).

## Adding a new IC

1. Create a directory `ic-library/<vendor>_<part>/`.
2. Write `manifest.yaml` (see existing ICs for examples).
3. Add `icon.svg` (any reasonable SVG, ~64×64).
4. Validate: `python tools/validate-manifest.py
   ic-library/<vendor>_<part>/manifest.yaml`.
5. If the IC is not a `firmware_host`, write the behavior:
   - Add a module under `simulator/crates/sim-behaviors/src/`.
   - Implement the `IcBehavior` trait.
   - Register it in `sim-behaviors/src/lib.rs`.
6. Add a unit test for the behavior.

## Current ICs

| ID                       | Kind           | Notes                                    |
|--------------------------|----------------|------------------------------------------|
| `st/stm32f4`             | firmware_host  | MCU boundary; no behavior code.          |
| `bosch/bme280`           | sensor         | I2C, subscribes to temp/humidity/pressure. |
| `microchip/mcp23017`     | io_expander    | I2C ↔ 16 GPIO pins.                      |
| `generic/gpio_led`       | actuator       | Single GPIO LED, publishes state.        |
