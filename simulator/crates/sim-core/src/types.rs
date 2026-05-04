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
/// Uses borrows — see `OwnedBusTransaction` for heap-storable equivalent.
#[derive(Debug)]
pub enum BusTransaction<'a> {
    I2cWrite { address: u8, data: &'a [u8] },
    I2cRead  { address: u8, length: usize },
    I2cWriteRead { address: u8, write: &'a [u8], read_length: usize },
    SpiTransfer { mosi: &'a [u8] /* miso filled in BusResponse::Data */ },
    UartFrame { data: &'a [u8] },
}

/// Heap-storable bus transaction — same variants as `BusTransaction` but with
/// owned data. Convert to `BusTransaction<'_>` via `as_transaction()`.
#[derive(Debug, Clone)]
pub enum OwnedBusTransaction {
    I2cWrite { address: u8, data: Vec<u8> },
    I2cRead  { address: u8, length: usize },
    I2cWriteRead { address: u8, write: Vec<u8>, read_length: usize },
    SpiTransfer { mosi: Vec<u8> },
    UartFrame { data: Vec<u8> },
}

impl OwnedBusTransaction {
    /// Borrow as a `BusTransaction<'_>`, with lifetimes tied to `self`.
    pub fn as_transaction(&self) -> BusTransaction<'_> {
        match self {
            Self::I2cWrite { address, data } =>
                BusTransaction::I2cWrite { address: *address, data },
            Self::I2cRead { address, length } =>
                BusTransaction::I2cRead { address: *address, length: *length },
            Self::I2cWriteRead { address, write, read_length } =>
                BusTransaction::I2cWriteRead { address: *address, write, read_length: *read_length },
            Self::SpiTransfer { mosi } =>
                BusTransaction::SpiTransfer { mosi },
            Self::UartFrame { data } =>
                BusTransaction::UartFrame { data },
        }
    }

    /// Extract the I2C address if this is an I2C variant; `None` otherwise.
    pub fn i2c_address(&self) -> Option<u8> {
        match self {
            Self::I2cWrite { address, .. } => Some(*address),
            Self::I2cRead { address, .. } => Some(*address),
            Self::I2cWriteRead { address, .. } => Some(*address),
            _ => None,
        }
    }
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

/// A config value from the board YAML's `component.config` block.
/// Covers the three types a manifest config field can have: string, number, bool.
#[derive(Debug, Clone)]
pub enum ConfigValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl ConfigValue {
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self { Some(s) } else { None }
    }
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(n) = self { Some(*n) } else { None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self { Some(*b) } else { None }
    }
}

/// GPIO pin direction — used in `IpcOperation::GpioConfigure`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GpioDirection {
    In,
    Out,
}

/// GPIO pull resistor configuration — used in `IpcOperation::GpioConfigure`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GpioPull {
    None,
    Up,
    Down,
}
