//! SPI bus router.
//!
//! Slaves are selected by the master's CS pin, not by an address. The
//! board YAML must declare each slave's CS pin on the master.
//!
//! Full-duplex: MOSI bytes in, MISO bytes out, same length.

use crate::types::{BusId, ComponentId};
use std::collections::HashMap;

pub struct SpiRouter {
    pub bus_id: BusId,
    pub master: ComponentId,
    /// CS pin name on master → which slave it selects.
    pub slaves_by_cs: HashMap<String, ComponentId>,
}

impl SpiRouter {
    // TODO: pub fn new(...) -> Self
    // TODO: pub fn dispatch(...) -> BusResponse
}
