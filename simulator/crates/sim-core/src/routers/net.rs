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
    /// Reset to false when contention clears.
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
    pub fn new(net_id: u32, endpoints: Vec<NetEndpoint>) -> Self {
        let len = endpoints.len();
        Self {
            net_id,
            endpoints,
            driven_values: vec![PinValue::HighZ; len],
            resolved: PinValue::HighZ,
            contention_warned: false,
        }
    }

    /// Drive endpoint `endpoint_idx` to `value`, re-resolve the net, and
    /// return what changed (if anything).
    pub fn drive(&mut self, endpoint_idx: usize, value: PinValue) -> ResolutionResult {
        self.driven_values[endpoint_idx] = value;
        self.resolve()
    }

    fn resolve(&mut self) -> ResolutionResult {
        let old = self.resolved;

        let non_z: Vec<PinValue> = self.driven_values.iter()
            .copied()
            .filter(|&v| v != PinValue::HighZ)
            .collect();

        let new_val = if non_z.is_empty() {
            self.contention_warned = false;
            PinValue::HighZ
        } else {
            let first = non_z[0];
            if non_z.iter().all(|&v| v == first) {
                self.contention_warned = false;
                first
            } else {
                // Contention: warn once, resolve to LOW.
                if !self.contention_warned {
                    self.contention_warned = true;
                    tracing::warn!(
                        net_id = self.net_id,
                        "net contention: multiple conflicting drivers, resolving to LOW"
                    );
                }
                PinValue::Low
            }
        };

        self.resolved = new_val;
        let changed = new_val != old;

        let endpoints_to_notify = if changed {
            self.endpoints.iter().enumerate()
                .filter(|(_, ep)| matches!(ep.pin_dir, PinDirection::In | PinDirection::Bidir))
                .map(|(i, _)| i)
                .collect()
        } else {
            Vec::new()
        };

        ResolutionResult { new_value: new_val, changed, endpoints_to_notify }
    }
}

pub struct ResolutionResult {
    pub new_value: PinValue,
    pub changed: bool,
    /// Indices into `Net::endpoints` that should receive `on_pin_change`.
    pub endpoints_to_notify: Vec<usize>,
}
