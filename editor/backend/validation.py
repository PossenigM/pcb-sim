"""JSON Schema validation for board YAML and IC manifests, plus
cross-validation rules that JSON Schema cannot express (component
reference integrity, I2C address conflicts, pin double-claims).
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import TYPE_CHECKING, Any

import jsonschema

if TYPE_CHECKING:
    from .library import IcLibrary


def _load_schema(schema_dir: Path, name: str) -> dict[str, Any]:
    with (schema_dir / name).open() as f:
        return json.load(f)


def validate_manifest_yaml(payload: dict[str, Any], *, schema_dir: Path) -> list[str]:
    """Return a list of error messages. Empty list means valid."""
    schema = _load_schema(schema_dir, "ic-manifest.schema.json")
    return _run_jsonschema(payload, schema)


def validate_board_yaml(payload: dict[str, Any], *, schema_dir: Path) -> list[str]:
    """JSON Schema validation only. Call cross_validate_board separately."""
    schema = _load_schema(schema_dir, "board.schema.json")
    return _run_jsonschema(payload, schema)


def cross_validate_board(board: dict[str, Any], library: "IcLibrary | None") -> list[str]:
    """Checks beyond what JSON Schema can express:

    1. Every component ID referenced in buses/nets exists in ``components``.
    2. No interface is claimed by more than one bus.
    3. No pin is claimed by more than one net.
    4. No two I2C slaves on the same bus share an address.
    """
    errors: list[str] = []

    components: dict[str, dict[str, Any]] = {
        c["id"]: c
        for c in board.get("components", [])
        if isinstance(c, dict) and c.get("id")
    }

    # claimed_interfaces[(comp_id, iface_name)] = bus_id
    claimed_interfaces: dict[tuple[str, str], str] = {}
    # claimed_pins[(comp_id, pin_name)] = net_id
    claimed_pins: dict[tuple[str, str], str] = {}

    for bus in board.get("buses", []):
        if not isinstance(bus, dict):
            continue
        bus_id = bus.get("id", "<unnamed bus>")
        protocol = bus.get("protocol", "")
        members = _bus_members(bus, protocol)

        for member in members:
            comp_id = member.get("component")
            iface = member.get("interface")
            if comp_id and comp_id not in components:
                errors.append(f"bus '{bus_id}': component '{comp_id}' is not defined")
            if comp_id and iface:
                key = (comp_id, iface)
                if key in claimed_interfaces:
                    errors.append(
                        f"bus '{bus_id}': {comp_id}.{iface} is already used"
                        f" in bus '{claimed_interfaces[key]}'"
                    )
                else:
                    claimed_interfaces[key] = bus_id

    for net in board.get("nets", []):
        if not isinstance(net, dict):
            continue
        net_id = net.get("id", "<unnamed net>")
        for ep in net.get("endpoints", []):
            comp_id = ep.get("component")
            pin = ep.get("pin")
            if comp_id and comp_id not in components:
                errors.append(f"net '{net_id}': component '{comp_id}' is not defined")
            if comp_id and pin:
                key = (comp_id, pin)
                if key in claimed_pins:
                    errors.append(
                        f"net '{net_id}': {comp_id}.{pin} is already used"
                        f" in net '{claimed_pins[key]}'"
                    )
                else:
                    claimed_pins[key] = net_id

    _check_i2c_address_conflicts(board, components, library, errors)

    return errors


def _bus_members(bus: dict[str, Any], protocol: str) -> list[dict[str, Any]]:
    """Return all component/interface references for any bus protocol."""
    if protocol == "i2c":
        return [m for m in bus.get("members", []) if isinstance(m, dict)]
    if protocol == "spi":
        result = []
        if isinstance(bus.get("master"), dict):
            result.append(bus["master"])
        result.extend(s for s in bus.get("slaves", []) if isinstance(s, dict))
        return result
    if protocol == "uart":
        return [p for p in bus.get("peers", []) if isinstance(p, dict)]
    return []


def _check_i2c_address_conflicts(
    board: dict[str, Any],
    components: dict[str, dict[str, Any]],
    library: "IcLibrary | None",
    errors: list[str],
) -> None:
    """Emit an error for each pair of I2C slaves that share an address on the same bus."""
    for bus in board.get("buses", []):
        if not isinstance(bus, dict) or bus.get("protocol") != "i2c":
            continue
        bus_id = bus.get("id", "<unnamed bus>")

        # address_value -> first comp_id that claimed it
        seen: dict[Any, str] = {}
        for member in bus.get("members", []):
            if not isinstance(member, dict):
                continue
            comp_id = member.get("component")
            if not comp_id or comp_id not in components:
                continue
            comp = components[comp_id]
            addr = _resolve_i2c_address(comp, library)
            if addr is None:
                continue
            addr_repr = hex(addr) if isinstance(addr, int) else str(addr)
            if addr in seen:
                errors.append(
                    f"bus '{bus_id}': I2C address {addr_repr} conflict"
                    f" between '{seen[addr]}' and '{comp_id}'"
                )
            else:
                seen[addr] = comp_id


def _resolve_i2c_address(
    comp: dict[str, Any], library: "IcLibrary | None"
) -> Any | None:
    """Return the effective I2C address for a component, or None if unknown."""
    config = comp.get("config") or {}
    addr = config.get("i2c.address")
    if addr is not None:
        return addr

    if library is None:
        return None

    comp_type = comp.get("type", "")
    entry = library.get(comp_type)
    if entry is None:
        return None

    for iface in entry.manifest.get("interfaces", []):
        if iface.get("protocol") == "i2c":
            iface_config = iface.get("config") or {}
            addr_cfg = iface_config.get("address") or {}
            default = addr_cfg.get("default")
            if default is not None:
                return default

    return None


def _run_jsonschema(payload: dict[str, Any], schema: dict[str, Any]) -> list[str]:
    validator = jsonschema.Draft202012Validator(schema)
    errors: list[str] = []
    for err in sorted(validator.iter_errors(payload), key=lambda e: e.path):
        path = "/".join(str(p) for p in err.absolute_path) or "<root>"
        errors.append(f"{path}: {err.message}")
    return errors
