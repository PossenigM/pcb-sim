//! IC manifest types and loader. Mirrors `schema/ic-manifest.schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcManifest {
    pub id: String,
    pub version: String,
    pub kind: IcKind,
    #[serde(default)]
    pub description: Option<String>,
    pub interfaces: Vec<Interface>,
    pub pins: Vec<Pin>,
    #[serde(default)]
    pub config: BTreeMap<String, ConfigField>,
    #[serde(default)]
    pub mqtt: Option<MqttBlock>,
    #[serde(default)]
    pub behavior: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IcKind {
    FirmwareHost,
    Sensor,
    Actuator,
    IoExpander,
    Logic,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub role: Option<InterfaceRole>,
    pub pins: Vec<String>,
    #[serde(default)]
    pub config: BTreeMap<String, ConfigField>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    I2c,
    Spi,
    Uart,
    Gpio,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceRole {
    Master,
    Slave,
    Peer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub name: String,
    pub dir: PinDir,
    #[serde(default)]
    pub default: Option<PinDefault>,
    #[serde(default)]
    pub runtime_configurable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PinDir {
    In,
    Out,
    Bidir,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum PinDefault {
    LOW,
    HIGH,
    Z,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    #[serde(rename = "type")]
    pub kind: ConfigFieldType,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    #[serde(default)]
    pub choices: Option<Vec<serde_yaml::Value>>,
    #[serde(default)]
    pub range: Option<Vec<serde_yaml::Value>>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFieldType {
    Int,
    Float,
    String,
    Bool,
    Hex,
    Enum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttBlock {
    #[serde(default)]
    pub publish: Vec<MqttChannel>,
    #[serde(default)]
    pub subscribe: Vec<MqttChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttChannel {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: MqttPayloadType,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MqttPayloadType {
    Bool,
    Int,
    Float,
    String,
    Bytes,
}

impl IcManifest {
    /// Load an IC manifest from a YAML file, validating against the JSON
    /// Schema before returning.
    pub fn load_yaml_file(path: &std::path::Path) -> Result<Self, crate::validate::ValidationError> {
        let text = std::fs::read_to_string(path).map_err(|e| crate::validate::ValidationError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Self::from_yaml_str(&text)
    }

    /// Parse an IC manifest from a YAML string, validating against the JSON
    /// Schema before returning.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, crate::validate::ValidationError> {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
        crate::validate::validate_manifest(&value)?;
        let manifest: Self = serde_yaml::from_value(value)?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BME280_YAML: &str =
        include_str!("../../../../ic-library/bosch_bme280/manifest.yaml");
    const STM32F4_YAML: &str =
        include_str!("../../../../ic-library/st_stm32f4/manifest.yaml");
    const MCP23017_YAML: &str =
        include_str!("../../../../ic-library/microchip_mcp23017/manifest.yaml");

    #[test]
    fn bme280_manifest_loads() {
        let m = IcManifest::from_yaml_str(BME280_YAML).unwrap();
        assert_eq!(m.id, "bosch/bme280");
        assert_eq!(m.version, "0.1");
        assert_eq!(m.kind, IcKind::Sensor);
        assert!(m.mqtt.is_some());
        assert_eq!(m.behavior.as_deref(), Some("builtin:bme280"));
    }

    #[test]
    fn stm32f4_is_firmware_host() {
        let m = IcManifest::from_yaml_str(STM32F4_YAML).unwrap();
        assert_eq!(m.kind, IcKind::FirmwareHost);
        assert!(m.behavior.is_none());
        assert!(m.interfaces.iter().any(|i| i.name == "i2c1"));
    }

    #[test]
    fn mcp23017_has_i2c_interface_with_address_config() {
        let m = IcManifest::from_yaml_str(MCP23017_YAML).unwrap();
        let iface = m.interfaces.iter().find(|i| i.name == "i2c").unwrap();
        assert_eq!(iface.role, Some(InterfaceRole::Slave));
        assert!(iface.config.contains_key("address"));
    }
}
