//! Board definition types and loader. Mirrors `schema/board.schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// `vendor/part@version`.
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

impl Board {
    pub fn load_yaml_file(_path: &std::path::Path) -> Result<Self, crate::validate::ValidationError> {
        // TODO: read file → parse YAML → validate via schema → deserialize
        unimplemented!()
    }

    pub fn from_yaml_str(_yaml: &str) -> Result<Self, crate::validate::ValidationError> {
        // TODO: parse → validate → deserialize
        unimplemented!()
    }

    /// Cross-reference checks beyond what JSON Schema can express:
    ///   - every component.type resolves in the IC library
    ///   - I2C slave addresses are unique per bus
    ///   - SPI CS pins are unique per master
    ///   - no pin appears in both a bus interface and a net
    ///   - sensor MQTT subscriptions have initial_values
    ///   - MQTT mappings reference channels declared in the manifest
    ///   - exactly one I2C master per bus (v1)
    pub fn cross_validate(&self, _library: &crate::library::IcLibrary)
        -> Result<(), crate::validate::ValidationError>
    {
        // TODO: implement the checks listed above.
        unimplemented!()
    }
}
