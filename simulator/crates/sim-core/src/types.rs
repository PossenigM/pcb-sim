//! Core value types used throughout the simulator.

use std::fmt;

/// A simulated logical pin value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PinValue {
    Low,
    High,
    /// High-impedance: pin is not actively driving.
    HighZ,
}

impl fmt::Display for PinValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::High => write!(f, "HIGH"),
            Self::HighZ => write!(f, "Z"),
        }
    }
}

/// Resolved at simulator init time from a (component_id, pin_name) pair.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PinId(pub u32);

/// Resolved at simulator init time from a bus id string.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BusId(pub u32);

/// Resolved at simulator init time from a component id string.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ComponentId(pub u32);

/// A bus transaction. The variant matches the bus protocol.
#[derive(Debug)]
pub enum BusTransaction<'a> {
    I2cWrite { address: u8, data: &'a [u8] },
    I2cRead  { address: u8, length: usize },
    I2cWriteRead { address: u8, write: &'a [u8], read_length: usize },
    SpiTransfer { mosi: &'a [u8] /* miso filled in BusResponse::Data */ },
    UartFrame { data: &'a [u8] },
}

#[derive(Debug, Clone)]
pub enum BusResponse {
    /// Write completed (no return data).
    None,
    /// Read or transfer produced this data.
    Data(Vec<u8>),
    /// I2C address not acknowledged (or equivalent).
    Nack,
}

/// MQTT payload — strongly typed so behaviors don't parse strings.
#[derive(Debug, Clone)]
pub enum MqttValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}
