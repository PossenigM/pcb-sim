//! Wire message types. See `docs/wire-protocol.md` for the spec.
//!
//! Every message is tagged with a `type` field for serde discrimination,
//! and may carry an optional `meta` map for forward-compatible extensions.

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;

/// All firmware → simulator messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        protocol_version: u32,
        client: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    I2cWrite {
        id: i64,
        bus: String,
        address: u8,
        data: ByteBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    I2cRead {
        id: i64,
        bus: String,
        address: u8,
        length: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    I2cWriteRead {
        id: i64,
        bus: String,
        address: u8,
        write: ByteBuf,
        read_length: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    SpiTransfer {
        id: i64,
        bus: String,
        cs: String,
        mosi: ByteBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioWrite {
        id: i64,
        pin: String,
        value: PinValueWire,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioRead {
        id: i64,
        pin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioConfigure {
        id: i64,
        pin: String,
        direction: PinDirectionWire,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pull: Option<PinPullWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    UartTx {
        bus: String,
        data: ByteBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
}

/// All simulator → firmware messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    HelloAck {
        protocol_version: u32,
        mcu_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    I2cAck {
        id: i64,
        result: AckResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    I2cData {
        id: i64,
        result: AckResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<ByteBuf>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    SpiData {
        id: i64,
        result: AckResult,
        miso: ByteBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioAck {
        id: i64,
        result: AckResult,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioValue {
        id: i64,
        result: AckResult,
        value: PinValueWire,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    GpioEvent {
        pin: String,
        value: PinValueWire,
        sim_time_us: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    UartRx {
        bus: String,
        data: ByteBuf,
        sim_time_us: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    UartOverflowEvent {
        bus: String,
        dropped_bytes: u32,
        sim_time_us: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    SimulatorEvent {
        event: SimulatorEventKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        sim_time_us: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<i64>,
        code: ErrorCode,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PinValueWire {
    Low,
    High,
    Z,
    /// Integer-valued analog/PWM signal (e.g. duty cycle 0–255 or RPM).
    Analog(u32),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PinDirectionWire {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PinPullWire {
    None,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AckResult {
    Ok,
    Nack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorEventKind {
    Shutdown,
    Reset,
    Warning,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedVersion,
    NoSuchBus,
    NoSuchPin,
    BadRequest,
    InternalError,
}

/// Forward-compatible extension map. Receivers MUST ignore unknown keys.
///
/// TODO: replace `String` value type with a richer dynamic value (e.g.
/// `rmpv::Value`) once a use case appears. For now, string values are
/// sufficient and avoid an extra dependency.
pub type Meta = BTreeMap<String, String>;
