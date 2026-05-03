# Tools

Small Python CLI utilities for offline validation. Useful in CI and as
sanity checks while iterating on the schemas.

## Setup

```bash
pip install jsonschema pyyaml
```

## Validate an IC manifest

```bash
python tools/validate-manifest.py ic-library/bosch_bme280/manifest.yaml
```

Exit code 0 = valid. Non-zero = errors printed to stderr.

## Validate a board YAML

```bash
python tools/validate-board.py examples/dev_board_v1.yaml
```

Performs JSON Schema validation only. Cross-validation against the IC
library (address conflicts, missing channels, etc.) is TODO.

## Manual firmware client (TODO)

A `manual_firmware_client.py` for sending raw IPC messages to a running
simulator while developing. Useful for testing the wire protocol before
your real firmware HAL is ready.
