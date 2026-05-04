//! Board definition types and loader. Mirrors `schema/board.schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub board: BoardHeader,
    #[serde(default)]
    pub layout: BTreeMap<String, LayoutPos>,
    #[serde(default)]
    pub mqtt: Option<MqttSettings>,
    pub components: Vec<Component>,
    pub buses: Vec<Bus>,
    pub nets: Vec<Net>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardHeader {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LayoutPos {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttSettings {
    #[serde(default)]
    pub broker: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    /// `vendor/part@version` (e.g. `bosch/bme280@0.1`).
    #[serde(rename = "type")]
    pub type_ref: String,
    #[serde(default)]
    pub config: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub initial_values: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub mqtt: Option<ComponentMqtt>,
    #[serde(default)]
    pub firmware: Option<FirmwareEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMqtt {
    /// channel name → topic (relative to board prefix)
    #[serde(default)]
    pub publish: BTreeMap<String, String>,
    /// channel name → topic
    #[serde(default)]
    pub subscribe: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareEndpoint {
    pub transport: FirmwareTransport,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareTransport {
    UnixSocket,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum Bus {
    I2c {
        id: String,
        members: Vec<I2cMember>,
    },
    Spi {
        id: String,
        master: SpiMaster,
        slaves: Vec<SpiSlave>,
    },
    Uart {
        id: String,
        peers: Vec<UartPeer>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2cMember {
    pub component: String,
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiMaster {
    pub component: String,
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiSlave {
    pub component: String,
    pub interface: String,
    pub cs_pin_on_master: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UartPeer {
    pub component: String,
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Net {
    pub id: String,
    pub endpoints: Vec<NetEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetEndpoint {
    pub component: String,
    pub pin: String,
}

// ─── loader ──────────────────────────────────────────────────────────────────

impl Board {
    /// Load a board definition from a YAML file, validating against the JSON
    /// Schema before returning.
    pub fn load_yaml_file(path: &std::path::Path) -> Result<Self, crate::validate::ValidationError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            crate::validate::ValidationError::Io {
                path: path.display().to_string(),
                source: e,
            }
        })?;
        Self::from_yaml_str(&text)
    }

    /// Parse a board definition from a YAML string, validating against the
    /// JSON Schema before returning.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, crate::validate::ValidationError> {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
        crate::validate::validate_board(&value)?;
        let board: Self = serde_yaml::from_value(value)?;
        Ok(board)
    }

    // ─── cross-validation ────────────────────────────────────────────────────

    /// Semantic checks that go beyond what JSON Schema can express.
    ///
    /// Checks performed:
    /// - Every `component.type` resolves in the IC library.
    /// - Exactly one I2C master per bus (v1 constraint).
    /// - I2C slave addresses are unique per bus.
    /// - SPI CS pins are unique per master.
    /// - No pin appears in both a bus interface and a net.
    /// - MQTT channel names in the board YAML match channels declared in the manifest.
    pub fn cross_validate(
        &self,
        library: &crate::library::IcLibrary,
    ) -> Result<(), crate::validate::ValidationError> {
        use crate::validate::ValidationError;

        let mut errors: Vec<String> = Vec::new();

        // Index components by id for quick lookup.
        let comp_by_id: HashMap<&str, &Component> =
            self.components.iter().map(|c| (c.id.as_str(), c)).collect();

        // ── 1. Resolve all component types ───────────────────────────────────
        let mut manifest_by_comp: HashMap<&str, &crate::manifest::IcManifest> = HashMap::new();
        for comp in &self.components {
            match library.lookup(&comp.type_ref) {
                Some(entry) => {
                    manifest_by_comp.insert(comp.id.as_str(), &entry.manifest);
                }
                None => errors.push(format!(
                    "component '{}': unknown type '{}' (not found in IC library)",
                    comp.id, comp.type_ref
                )),
            }
        }

        // If some types are unresolvable we can't do deeper checks that need
        // the manifest, so bail early.
        if !errors.is_empty() {
            return Err(ValidationError::CrossValidation(errors.join("\n")));
        }

        // ── 2. Bus checks + build set of bus-owned (component, pin) pairs ────
        let mut bus_pins: HashSet<(String, String)> = HashSet::new();

        for bus in &self.buses {
            match bus {
                Bus::I2c { id: bus_id, members } => {
                    check_i2c_bus(
                        bus_id,
                        members,
                        &comp_by_id,
                        &manifest_by_comp,
                        &mut bus_pins,
                        &mut errors,
                    );
                }
                Bus::Spi { id: bus_id, master, slaves } => {
                    check_spi_bus(
                        bus_id,
                        master,
                        slaves,
                        &comp_by_id,
                        &manifest_by_comp,
                        &mut bus_pins,
                        &mut errors,
                    );
                }
                Bus::Uart { id: bus_id, peers } => {
                    check_uart_bus(
                        bus_id,
                        peers,
                        &comp_by_id,
                        &manifest_by_comp,
                        &mut bus_pins,
                        &mut errors,
                    );
                }
            }
        }

        // ── 3. Net pins must not overlap with bus-owned pins ─────────────────
        for net in &self.nets {
            for ep in &net.endpoints {
                let key = (ep.component.clone(), ep.pin.clone());
                if bus_pins.contains(&key) {
                    errors.push(format!(
                        "net '{}': pin '{}' on component '{}' is already claimed by a bus interface",
                        net.id, ep.pin, ep.component
                    ));
                }
            }
        }

        // ── 4. MQTT channel names must match manifest declarations ────────────
        for comp in &self.components {
            let manifest = manifest_by_comp[comp.id.as_str()];
            let Some(comp_mqtt) = &comp.mqtt else { continue };
            let mqtt_block = manifest.mqtt.as_ref();

            for channel in comp_mqtt.publish.keys() {
                let declared = mqtt_block
                    .map(|m| m.publish.iter().any(|c| &c.name == channel))
                    .unwrap_or(false);
                if !declared {
                    errors.push(format!(
                        "component '{}': mqtt.publish channel '{}' is not declared in manifest '{}'",
                        comp.id, channel, comp.type_ref
                    ));
                }
            }

            for channel in comp_mqtt.subscribe.keys() {
                let declared = mqtt_block
                    .map(|m| m.subscribe.iter().any(|c| &c.name == channel))
                    .unwrap_or(false);
                if !declared {
                    errors.push(format!(
                        "component '{}': mqtt.subscribe channel '{}' is not declared in manifest '{}'",
                        comp.id, channel, comp.type_ref
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::CrossValidation(errors.join("\n")))
        }
    }
}

// ─── bus check helpers ────────────────────────────────────────────────────────

fn check_i2c_bus(
    bus_id: &str,
    members: &[I2cMember],
    comp_by_id: &HashMap<&str, &Component>,
    manifest_by_comp: &HashMap<&str, &crate::manifest::IcManifest>,
    bus_pins: &mut HashSet<(String, String)>,
    errors: &mut Vec<String>,
) {
    use crate::manifest::InterfaceRole;

    let mut master_count: usize = 0;
    // (i2c_address, component_id) — track for duplicate detection.
    let mut slave_addrs: HashMap<i64, String> = HashMap::new();

    for member in members {
        let Some(manifest) = manifest_by_comp.get(member.component.as_str()) else {
            continue; // type resolution already failed, skip
        };

        let Some(iface) = manifest.interfaces.iter().find(|i| i.name == member.interface) else {
            errors.push(format!(
                "I2C bus '{}': component '{}' has no interface named '{}'",
                bus_id, member.component, member.interface
            ));
            continue;
        };

        // Claim the interface's pins for the bus router.
        for pin in &iface.pins {
            bus_pins.insert((member.component.clone(), pin.clone()));
        }

        match iface.role {
            Some(InterfaceRole::Master) => master_count += 1,
            Some(InterfaceRole::Slave) | None => {
                // Resolve the I2C address (board config overrides manifest default).
                let comp = comp_by_id[member.component.as_str()];
                let addr = resolve_i2c_address(comp, iface);
                if let Some(a) = addr {
                    if let Some(existing) = slave_addrs.insert(a, member.component.clone()) {
                        errors.push(format!(
                            "I2C bus '{}': address 0x{a:02X} used by both '{}' and '{}'",
                            bus_id, existing, member.component
                        ));
                    }
                }
            }
            Some(InterfaceRole::Peer) => {
                errors.push(format!(
                    "I2C bus '{}': component '{}' interface '{}' has role 'peer', \
                     which is not valid for I2C",
                    bus_id, member.component, member.interface
                ));
            }
        }
    }

    if master_count != 1 {
        errors.push(format!(
            "I2C bus '{}' must have exactly 1 master, found {master_count}",
            bus_id
        ));
    }
}

fn check_spi_bus(
    bus_id: &str,
    master: &SpiMaster,
    slaves: &[SpiSlave],
    comp_by_id: &HashMap<&str, &Component>,
    manifest_by_comp: &HashMap<&str, &crate::manifest::IcManifest>,
    bus_pins: &mut HashSet<(String, String)>,
    errors: &mut Vec<String>,
) {
    let _ = comp_by_id; // not needed here but kept for symmetry

    // Claim master interface pins.
    if let Some(manifest) = manifest_by_comp.get(master.component.as_str()) {
        if let Some(iface) = manifest.interfaces.iter().find(|i| i.name == master.interface) {
            for pin in &iface.pins {
                bus_pins.insert((master.component.clone(), pin.clone()));
            }
        } else {
            errors.push(format!(
                "SPI bus '{}': master component '{}' has no interface named '{}'",
                bus_id, master.component, master.interface
            ));
        }
    }

    // Check CS pins are unique and claim slave interface pins.
    let mut cs_seen: HashMap<&str, &str> = HashMap::new();
    for slave in slaves {
        let cs = slave.cs_pin_on_master.as_str();
        if let Some(existing) = cs_seen.insert(cs, slave.component.as_str()) {
            errors.push(format!(
                "SPI bus '{}': CS pin '{}' used by both '{}' and '{}'",
                bus_id, cs, existing, slave.component
            ));
        }

        if let Some(manifest) = manifest_by_comp.get(slave.component.as_str()) {
            if let Some(iface) = manifest.interfaces.iter().find(|i| i.name == slave.interface) {
                for pin in &iface.pins {
                    bus_pins.insert((slave.component.clone(), pin.clone()));
                }
            } else {
                errors.push(format!(
                    "SPI bus '{}': slave component '{}' has no interface named '{}'",
                    bus_id, slave.component, slave.interface
                ));
            }
        }
    }
}

fn check_uart_bus(
    bus_id: &str,
    peers: &[UartPeer],
    _comp_by_id: &HashMap<&str, &Component>,
    manifest_by_comp: &HashMap<&str, &crate::manifest::IcManifest>,
    bus_pins: &mut HashSet<(String, String)>,
    errors: &mut Vec<String>,
) {
    for peer in peers {
        if let Some(manifest) = manifest_by_comp.get(peer.component.as_str()) {
            if let Some(iface) = manifest.interfaces.iter().find(|i| i.name == peer.interface) {
                for pin in &iface.pins {
                    bus_pins.insert((peer.component.clone(), pin.clone()));
                }
            } else {
                errors.push(format!(
                    "UART bus '{}': component '{}' has no interface named '{}'",
                    bus_id, peer.component, peer.interface
                ));
            }
        }
    }
}

// ─── address helpers ──────────────────────────────────────────────────────────

/// Resolve the I2C address for a slave member.
///
/// The board component config key `<interface_name>.address` takes priority
/// over the manifest interface's `address.default`.
fn resolve_i2c_address(
    comp: &Component,
    iface: &crate::manifest::Interface,
) -> Option<i64> {
    let config_key = format!("{}.address", iface.name);

    // Board config override.
    if let Some(v) = comp.config.get(&config_key) {
        return yaml_as_i64(v);
    }

    // Manifest default.
    iface
        .config
        .get("address")
        .and_then(|cf| cf.default.as_ref())
        .and_then(yaml_as_i64)
}

/// Extract an integer from a `serde_yaml::Value`, handling both numeric
/// values and hex strings like `"0x76"` (YAML 1.2 core schema does not
/// guarantee hex parsing as integers).
fn yaml_as_i64(v: &serde_yaml::Value) -> Option<i64> {
    match v {
        serde_yaml::Value::Number(n) => n.as_i64(),
        serde_yaml::Value::String(s) => {
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()
            } else {
                s.parse().ok()
            }
        }
        _ => None,
    }
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const DEV_BOARD_YAML: &str = include_str!("../../../../examples/dev_board_v1.yaml");
    const MINIMAL_BLINK_YAML: &str = include_str!("../../../../examples/minimal_blink.yaml");

    #[test]
    fn dev_board_parses() {
        let board = Board::from_yaml_str(DEV_BOARD_YAML).unwrap();
        assert_eq!(board.board.name, "dev_board_v1");
        assert_eq!(board.components.len(), 5);
        assert_eq!(board.buses.len(), 1);
        assert_eq!(board.nets.len(), 2);
    }

    #[test]
    fn minimal_blink_parses() {
        Board::from_yaml_str(MINIMAL_BLINK_YAML).unwrap();
    }

    #[test]
    fn dev_board_cross_validates() {
        use crate::library::IcLibrary;
        let library_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ic-library");
        let lib = IcLibrary::load(&library_root).unwrap();
        let board = Board::from_yaml_str(DEV_BOARD_YAML).unwrap();
        board.cross_validate(&lib).unwrap();
    }

    #[test]
    fn cross_validate_rejects_unknown_type() {
        use crate::library::IcLibrary;
        let library_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ic-library");
        let lib = IcLibrary::load(&library_root).unwrap();

        let yaml = r#"
board:
  name: test
components:
  - id: u1
    type: unknown/part@0.1
buses: []
nets: []
"#;
        let board = Board::from_yaml_str(yaml).unwrap();
        let err = board.cross_validate(&lib).unwrap_err();
        assert!(err.to_string().contains("unknown type"));
    }

    #[test]
    fn cross_validate_rejects_duplicate_i2c_address() {
        use crate::library::IcLibrary;
        let library_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ic-library");
        let lib = IcLibrary::load(&library_root).unwrap();

        // Two BME280s on the same bus at the same address.
        let yaml = r#"
board:
  name: test
components:
  - id: mcu1
    type: st/stm32f4@0.1
  - id: s1
    type: bosch/bme280@0.1
    config:
      i2c.address: 0x76
  - id: s2
    type: bosch/bme280@0.1
    config:
      i2c.address: 0x76
buses:
  - id: bus0
    protocol: i2c
    members:
      - { component: mcu1, interface: i2c1 }
      - { component: s1,   interface: i2c }
      - { component: s2,   interface: i2c }
nets: []
"#;
        let board = Board::from_yaml_str(yaml).unwrap();
        let err = board.cross_validate(&lib).unwrap_err();
        assert!(err.to_string().contains("0x76"));
    }
}
