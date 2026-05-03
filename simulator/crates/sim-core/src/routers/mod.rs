//! Bus and net routers.
//!
//! Routers sit between the IPC adapter and IC behaviors. They handle
//! protocol-level dispatch (which slave gets this transaction, which net
//! got driven, etc.) and never touch I/O directly.

pub mod i2c;
pub mod net;
pub mod spi;
pub mod uart;
