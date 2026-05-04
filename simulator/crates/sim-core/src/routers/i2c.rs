//! I2C bus router.
//!
//! v1 supports a single master per bus (the IPC adapter). Multi-master
//! arbitration is out of scope.
//!
//! Behavior:
//!   1. Receive `OwnedBusTransaction` from a master.
//!   2. Look up the addressed slave in the address → component table.
//!   3. Return the `ComponentId` of the slave (or `None` for Nack).
//!
//! The event loop owns behaviors; the router just maps addresses to IDs
//! so the event loop can dispatch without borrow-checker gymnastics.
//!
//! Address conflicts are detected at simulator startup, not runtime;
//! the simulator refuses to start with a clear error message.

use crate::types::{BusId, ComponentId, OwnedBusTransaction};
use std::collections::HashMap;

pub struct I2cRouter {
    pub bus_id: BusId,
    /// I2C address (7-bit) → which component owns this address.
    pub slaves: HashMap<u8, ComponentId>,
    pub master: ComponentId,
}

impl I2cRouter {
    pub fn new(bus_id: BusId, master: ComponentId, slaves: HashMap<u8, ComponentId>) -> Self {
        Self { bus_id, master, slaves }
    }

    /// Look up which slave should handle `txn`. Returns `None` if the
    /// addressed slave is not registered (caller should respond with Nack).
    pub fn route(&self, txn: &OwnedBusTransaction) -> Option<ComponentId> {
        let addr = txn.i2c_address()?;
        self.slaves.get(&addr).copied()
    }
}
