# Example boards

Sample board YAML files. Useful for testing the simulator and as a
template for new boards.

## `minimal_blink.yaml`

The smallest meaningful board: an MCU and one LED on a GPIO. Use this for
the first end-to-end test of the simulator.

```bash
python ../tools/validate-board.py minimal_blink.yaml
../simulator/target/debug/sim-bin --board minimal_blink.yaml
```

## `dev_board_v1.yaml`

A more realistic board: MCU + I2C bus with a BME280 sensor and an MCP23017
expander, plus two LEDs (one direct on a GPIO, one via the expander).
Exercises I2C, GPIO nets, and MQTT publish/subscribe.

```bash
python ../tools/validate-board.py dev_board_v1.yaml
../simulator/target/debug/sim-bin --board dev_board_v1.yaml
```
