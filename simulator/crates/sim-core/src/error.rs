//! Error types for the simulator core.

use thiserror::Error;

/// Errors raised by IC behavior implementations.
#[derive(Debug, Error)]
pub enum IcError {
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),

    #[error("bad config: {0}")]
    BadConfig(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Errors raised by simulator infrastructure (config loading, routing, etc.).
#[derive(Debug, Error)]
pub enum SimError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("address conflict on bus {bus}: address {address:#04x} used by both {a} and {b}")]
    AddressConflict { bus: String, address: u8, a: String, b: String },

    #[error("pin {pin} on component {component} is in both a bus interface and a net")]
    PinDoubleClaim { component: String, pin: String },

    #[error("unknown component {0}")]
    UnknownComponent(String),

    #[error("unknown bus {0}")]
    UnknownBus(String),

    #[error("ic error: {0}")]
    Ic(#[from] IcError),
}
