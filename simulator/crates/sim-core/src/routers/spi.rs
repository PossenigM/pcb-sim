//! SPI bus router.
//!
//! Slaves are selected by the master's CS pin, not by an address. The
//! board YAML must declare each slave's CS pin on the master.
//!
//! Full-duplex: MOSI bytes in, MISO bytes out, same length.
//!
//! CS tracking: the event loop records which CS pin name is currently
//! driven low and passes it here at dispatch time. The router just maps
//! CS pin names to `ComponentId`s.

use crate::types::{BusId, ComponentId};
use std::collections::HashMap;

pub struct SpiRouter {
    pub bus_id: BusId,
    pub master: ComponentId,
    /// CS pin name on master → which slave it selects.
    pub slaves_by_cs: HashMap<String, ComponentId>,
}

impl SpiRouter {
    pub fn new(
        bus_id: BusId,
        master: ComponentId,
        slaves_by_cs: HashMap<String, ComponentId>,
    ) -> Self {
        Self { bus_id, master, slaves_by_cs }
    }

    /// Look up the slave selected by the given CS pin name.
    /// Returns `None` if no slave is registered for that CS pin.
    pub fn route_cs(&self, cs_pin_name: &str) -> Option<ComponentId> {
        self.slaves_by_cs.get(cs_pin_name).copied()
    }
}
