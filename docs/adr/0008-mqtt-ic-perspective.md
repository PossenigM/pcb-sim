# ADR 0008 — MQTT direction is from the IC's perspective

## Context

The natural-language phrasing is ambiguous: a temperature sensor's
temperature could be "published" (the sensor reports it) or "subscribed"
(the sensor needs the value). We had to pick a convention and stick to
it.

## Decision

MQTT direction is from the **IC's** perspective:

- Sensors **subscribe** to topics published by the physics engine. The
  sensor consumes the value and presents it to firmware via its
  interface.
- Actuators **publish** to topics consumed by the physics engine (or
  other listeners).
- ICs that do both (smart switches, etc.) declare both directions
  independently.

## Consequences

- The convention matches how each IC's behavior code naturally reads:
  "I subscribe to my input, I publish my output."
- Manifests can be validated: a `kind: sensor` IC declaring `publish:`
  channels is a likely mistake, worth a warning.
- Topic strings live in the board YAML, not the manifest, so the same IC
  can be reused across boards with different MQTT layouts.
- Initial values for sensor subscriptions are **required** in the board
  YAML; this guarantees firmware never sees "no data yet".
