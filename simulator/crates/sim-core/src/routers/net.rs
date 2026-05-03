//! GPIO net router.
//!
//! Permissive resolution per ADR 0004:
//!   - all endpoints HighZ → net is HighZ
//!   - one non-Z driver → net takes that value
//!   - multiple drivers, same value → net takes that value
//!   - multiple drivers, different values → log a prominent warning,
//!     resolve to LOW, continue (deduplicate warnings per net to avoid
//!     log flooding).
//!
//! When a net's resolved value changes, all endpoints whose pin is
//! declared `dir: in` or `dir: bidir` receive `on_pin_change`.

use crate::types::{ComponentId, PinId, PinValue};

pub struct Net {
    pub net_id: u32,
    pub endpoints: Vec<NetEndpoint>,
    /// Currently-driven value for each endpoint (parallel to `endpoints`).
    pub driven_values: Vec<PinValue>,
    /// Currently resolved net value.
    pub resolved: PinValue,
    /// Whether we've already warned about contention on this net.
    pub contention_warned: bool,
}

pub struct NetEndpoint {
    pub component: ComponentId,
    pub pin: PinId,
    pub pin_dir: PinDirection,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PinDirection {
    In,
    Out,
    Bidir,
}

impl Net {
    // TODO: pub fn new(...) -> Self
    // TODO: pub fn drive(&mut self, endpoint_idx: usize, value: PinValue) -> ResolutionResult
    //   Returns whether the resolved value changed and which endpoints
    //   need to be notified.
    // TODO: pub fn resolve(&mut self) -> PinValue
    //   Apply the resolution rule and update `self.resolved`. Log
    //   contention warnings (deduplicated).
}

pub struct ResolutionResult {
    pub new_value: PinValue,
    pub changed: bool,
    pub endpoints_to_notify: Vec<usize>,
}
