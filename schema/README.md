# Schemas

JSON Schema definitions for IC manifests and board YAML files. Both the
editor (Python) and the simulator (Rust) validate against these. Single
source of truth.

## Files

- `ic-manifest.schema.json` — validates a single IC manifest.
- `board.schema.json` — validates a board definition.

## Validating

From Python (used by the editor and the `tools/` CLI):

```bash
pip install jsonschema pyyaml
python tools/validate-manifest.py ic-library/bosch_bme280/manifest.yaml
python tools/validate-board.py examples/dev_board_v1.yaml
```

From Rust (used by the simulator):

```rust
use jsonschema::JSONSchema;
let schema = JSONSchema::compile(&schema_json)?;
schema.validate(&yaml_as_json)?;
```

## Notes

- Schemas are **hand-written** in v1, not auto-generated. The cost of
  drift between schema and Rust types is manageable; the cost of having
  a schema that lies about the contract is much higher.
- These files are draft 2020-12 JSON Schema.
- The board schema uses `oneOf` discriminated by `protocol:` for
  per-protocol bus shapes (see ADR 0006).
