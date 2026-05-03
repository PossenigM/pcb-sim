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
    /// Load an IC manifest from a YAML file. Validates against the JSON
    /// Schema before returning.
    pub fn load_yaml_file(_path: &std::path::Path) -> Result<Self, crate::validate::ValidationError> {
        // TODO: read file → parse YAML → validate via schema → deserialize
        unimplemented!()
    }

    pub fn from_yaml_str(_yaml: &str) -> Result<Self, crate::validate::ValidationError> {
        // TODO: parse → validate → deserialize
        unimplemented!()
    }
}
