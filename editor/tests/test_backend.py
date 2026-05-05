"""Tests for the editor backend."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import yaml
from fastapi.testclient import TestClient

REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = REPO_ROOT / "schema"
LIBRARY_DIR = REPO_ROOT / "ic-library"
EXAMPLES_DIR = REPO_ROOT / "examples"


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def client():
    import os
    os.environ.setdefault("PCB_SIM_LIBRARY", str(LIBRARY_DIR))
    os.environ.setdefault("PCB_SIM_SCHEMA_DIR", str(SCHEMA_DIR))
    from backend.app import app
    with TestClient(app) as c:
        yield c


@pytest.fixture(scope="session")
def dev_board_yaml() -> str:
    return (EXAMPLES_DIR / "dev_board_v1.yaml").read_text()


@pytest.fixture(scope="session")
def dev_board_dict(dev_board_yaml) -> dict:
    return yaml.safe_load(dev_board_yaml)


# ---------------------------------------------------------------------------
# Library
# ---------------------------------------------------------------------------

class TestLibrary:
    def test_list_returns_known_ics(self, client):
        resp = client.get("/api/library")
        assert resp.status_code == 200
        ids = {entry["id"] for entry in resp.json()}
        assert "bosch/bme280" in ids
        assert "st/stm32f4" in ids
        assert "generic/gpio_led" in ids
        assert "microchip/mcp23017" in ids

    def test_get_ic_by_bare_id(self, client):
        resp = client.get("/api/library/bosch/bme280")
        assert resp.status_code == 200
        manifest = resp.json()
        assert manifest["id"] == "bosch/bme280"
        assert manifest["kind"] == "sensor"

    def test_get_ic_by_versioned_id(self, client):
        resp = client.get("/api/library/bosch/bme280@0.1")
        assert resp.status_code == 200
        assert resp.json()["id"] == "bosch/bme280"

    def test_get_ic_unknown_returns_404(self, client):
        resp = client.get("/api/library/vendor/nonexistent")
        assert resp.status_code == 404

    def test_icon_served_for_ic_with_icon(self, client):
        # Check whichever IC has an icon; skip if none do.
        resp = client.get("/api/library")
        for entry in resp.json():
            if entry.get("icon_url"):
                ic_id = entry["id"]
                icon_resp = client.get(f"/api/icons/{ic_id}")
                assert icon_resp.status_code == 200
                assert "svg" in icon_resp.headers["content-type"]
                return
        pytest.skip("no IC with an icon in the library")


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

class TestValidate:
    def test_accepts_example_board(self, client, dev_board_dict):
        resp = client.post("/api/validate", json={"model": dev_board_dict})
        assert resp.status_code == 200
        assert resp.json()["errors"] == []

    def test_rejects_missing_required_field(self, client):
        # A board missing the required `board` key.
        bad = {"components": [{"id": "x", "type": "a/b@0.1"}], "buses": [], "nets": []}
        resp = client.post("/api/validate", json={"model": bad})
        assert resp.status_code == 200
        errors = resp.json()["errors"]
        assert any("board" in e for e in errors)

    def test_detects_unknown_component_in_net(self, client, dev_board_dict):
        import copy
        board = copy.deepcopy(dev_board_dict)
        board["nets"].append({
            "id": "bad_net",
            "endpoints": [
                {"component": "ghost_comp", "pin": "PA0"},
                {"component": "mcu1", "pin": "PA1"},
            ],
        })
        resp = client.post("/api/validate", json={"model": board})
        assert resp.status_code == 200
        errors = resp.json()["errors"]
        assert any("ghost_comp" in e for e in errors)

    def test_detects_i2c_address_conflict(self, client, dev_board_dict):
        import copy
        board = copy.deepcopy(dev_board_dict)
        # Give ioexp1 and temp1 the same I2C address.
        for comp in board["components"]:
            if comp["id"] in ("ioexp1", "temp1"):
                comp.setdefault("config", {})["i2c.address"] = 0x20
        resp = client.post("/api/validate", json={"model": board})
        assert resp.status_code == 200
        errors = resp.json()["errors"]
        assert any("conflict" in e.lower() or "0x20" in e for e in errors)

    def test_detects_duplicate_net_pin(self, client, dev_board_dict):
        import copy
        board = copy.deepcopy(dev_board_dict)
        # Duplicate net_led2's mcu1.PA0 in a second net.
        board["nets"].append({
            "id": "net_dup",
            "endpoints": [
                {"component": "mcu1", "pin": "PA0"},
                {"component": "led1", "pin": "A"},
            ],
        })
        resp = client.post("/api/validate", json={"model": board})
        assert resp.status_code == 200
        errors = resp.json()["errors"]
        assert any("PA0" in e for e in errors)


# ---------------------------------------------------------------------------
# Export / import round-trip
# ---------------------------------------------------------------------------

class TestExportImport:
    def test_export_returns_yaml_string(self, client, dev_board_dict):
        resp = client.post("/api/export", json={"model": dev_board_dict})
        assert resp.status_code == 200
        yaml_str = resp.json()["yaml"]
        assert isinstance(yaml_str, str)
        # Must round-trip back to the same logical structure.
        parsed = yaml.safe_load(yaml_str)
        assert parsed["board"]["name"] == dev_board_dict["board"]["name"]

    def test_import_returns_model(self, client, dev_board_yaml):
        resp = client.post("/api/import", json={"yaml": dev_board_yaml})
        assert resp.status_code == 200
        model = resp.json()
        assert model["board"]["name"] == "dev_board_v1"
        # layout must be present for every component.
        comp_ids = {c["id"] for c in model["components"]}
        layout_ids = set(model.get("layout", {}).keys())
        assert comp_ids <= layout_ids

    def test_round_trip_preserves_components(self, client, dev_board_dict):
        # export → import → compare component IDs.
        export_resp = client.post("/api/export", json={"model": dev_board_dict})
        yaml_str = export_resp.json()["yaml"]
        import_resp = client.post("/api/import", json={"yaml": yaml_str})
        model = import_resp.json()
        original_ids = {c["id"] for c in dev_board_dict["components"]}
        roundtrip_ids = {c["id"] for c in model["components"]}
        assert original_ids == roundtrip_ids

    def test_import_invalid_yaml_returns_422(self, client):
        resp = client.post("/api/import", json={"yaml": "this: is: not: valid: yaml: {"})
        assert resp.status_code == 422

    def test_import_adds_layout_for_unlaid_components(self, client):
        board_yaml = yaml.dump({
            "board": {"name": "test"},
            "components": [
                {"id": "mcu1", "type": "st/stm32f4@0.1"},
                {"id": "led1", "type": "generic/gpio_led@0.1"},
            ],
            "buses": [],
            "nets": [],
        })
        resp = client.post("/api/import", json={"yaml": board_yaml})
        assert resp.status_code == 200
        model = resp.json()
        assert "mcu1" in model["layout"]
        assert "led1" in model["layout"]

    def test_export_key_order(self, client, dev_board_dict):
        resp = client.post("/api/export", json={"model": dev_board_dict})
        yaml_str = resp.json()["yaml"]
        # 'board:' should appear before 'components:' in the output.
        assert yaml_str.index("board:") < yaml_str.index("components:")


# ---------------------------------------------------------------------------
# Health
# ---------------------------------------------------------------------------

def test_health(client):
    resp = client.get("/health")
    assert resp.status_code == 200
    assert resp.json()["status"] == "ok"
    assert resp.json()["library_loaded"] is True
