//! I2C bus router.
//!
//! v1 supports a single master per bus (the IPC adapter). Multi-master
//! arbitration is out of scope.
//!
//! Behavior:
//!   1. Receive `BusTransaction` from a master.
//!   2. Look up the addressed slave in the address → component table.
//!   3. If found, dispatch to slave's `on_bus_transaction` and return
//!      its `BusResponse` to the master.
//!   4. If not found, return `BusResponse::Nack`.
//!
//! Address conflicts are detected at simulator startup, not runtime;
//! the simulator refuses to start with a clear error message.

use crate::types::{BusId, ComponentId};
use std::collections::HashMap;

pub struct I2cRouter {
    pub bus_id: BusId,
    /// I2C address (7-bit) → which component owns this address.
    pub slaves: HashMap<u8, ComponentId>,
    pub master: ComponentId,
}

impl I2cRouter {
    // TODO: pub fn new(...) -> Self
    // TODO: pub fn dispatch(...) -> BusResponse
    //   Takes a transaction and a way to call into IC behaviors.
}
