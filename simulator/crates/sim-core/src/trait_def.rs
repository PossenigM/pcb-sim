//! The `IcBehavior` trait — every simulated peripheral implements this.

use crate::error::IcError;
use crate::time::SimTime;
use crate::types::{
    BusId, BusResponse, BusTransaction, ComponentId, ConfigValue, MqttValue,
    OwnedBusTransaction, PinId, PinValue,
};
use std::collections::HashMap;
use std::time::Duration;

/// Implemented by every simulated peripheral. `firmware_host` components
/// do not implement this; they are wired to the IPC adapter directly.
///
/// Outputs flow through `RunCtx`, never as direct calls. This makes
/// behaviors deterministic and unit-testable.
pub trait IcBehavior: Send {
    /// Called once after construction. Config is supplied via `InitCtx`.
    /// Use this to set up any default pin states, schedule the first tick
    /// (if periodic behavior is needed), and prime internal state.
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError>;

    /// A pin connected to this IC changed state. Only fired for pins
    /// declared `dir: in` or `dir: bidir`.
    fn on_pin_change(
        &mut self,
        pin: PinId,
        value: PinValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        let _ = (pin, value, ctx);
        Ok(())
    }

    /// A bus transaction was directed at this IC. The router has already
    /// matched the address (I2C) or chip-select (SPI); this method
    /// receives only the payload.
    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        let _ = (txn, ctx);
        Err(IcError::Unsupported("bus transactions"))
    }

    /// An MQTT message arrived on a topic this IC subscribed to.
    /// `channel` is the manifest-declared channel name, NOT the full
    /// topic — the simulator handles topic ↔ channel mapping.
    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        let _ = (channel, payload, ctx);
        Ok(())
    }

    /// Periodic tick. ICs opt in by calling `ctx.schedule_tick()`. Most
    /// behaviors don't need this.
    fn on_tick(&mut self, now: SimTime, ctx: &mut RunCtx<'_>) -> Result<(), IcError> {
        let _ = (now, ctx);
        Ok(())
    }
}

// ── PendingOutputs ────────────────────────────────────────────────────────────

/// Outputs queued up by a behavior during a single handler call.
/// The event loop drains this after every `IcBehavior` method returns.
#[derive(Default)]
pub struct PendingOutputs {
    /// Pins to drive. Applied in order; last write wins if duplicated.
    pub pin_writes: Vec<(PinId, PinValue)>,
    /// MQTT channel publishes. Channel name (not topic) — the event loop
    /// resolves the channel → topic mapping from the board YAML.
    pub mqtt_publishes: Vec<(String, MqttValue)>,
    /// Behavior-initiated bus transmissions. In v1, rare (ICs typically
    /// respond; only masters initiate). Routed by the event loop.
    pub bus_transmits: Vec<(BusId, OwnedBusTransaction)>,
    /// If `Some`, schedule `on_tick` to fire after this delay. One-shot;
    /// set again from `on_tick` to repeat.
    pub tick_request: Option<Duration>,
}

// ── InitCtx ───────────────────────────────────────────────────────────────────

/// Provided to `IcBehavior::init`. Exposes component config, name
/// resolution, and the ability to set initial pin states and schedule
/// the first tick.
pub struct InitCtx<'a> {
    pub(crate) component_id: ComponentId,
    /// Pin name (local to this component) → global PinId.
    pin_map: &'a HashMap<String, PinId>,
    /// Interface name (local to this component) → global BusId.
    bus_map: &'a HashMap<String, BusId>,
    /// Config key → value from the board YAML `component.config` block.
    config: &'a HashMap<String, ConfigValue>,
    pub(crate) outputs: PendingOutputs,
}

impl<'a> InitCtx<'a> {
    pub(crate) fn new(
        component_id: ComponentId,
        pin_map: &'a HashMap<String, PinId>,
        bus_map: &'a HashMap<String, BusId>,
        config: &'a HashMap<String, ConfigValue>,
    ) -> Self {
        Self {
            component_id,
            pin_map,
            bus_map,
            config,
            outputs: PendingOutputs::default(),
        }
    }

    /// Resolve a pin name (e.g. `"gpio"`, `"sda"`) to its global `PinId`.
    /// Returns `None` if the name is not declared in the manifest.
    pub fn pin_id(&self, name: &str) -> Option<PinId> {
        self.pin_map.get(name).copied()
    }

    /// Resolve an interface name (e.g. `"i2c"`, `"spi1"`) to its global `BusId`.
    /// Returns `None` if the interface name is not known.
    pub fn bus_id(&self, interface: &str) -> Option<BusId> {
        self.bus_map.get(interface).copied()
    }

    /// Read a config value by key (from the board YAML `component.config` block).
    pub fn config_value(&self, key: &str) -> Option<&ConfigValue> {
        self.config.get(key)
    }

    /// Drive an output pin to an initial state. The net is updated after
    /// all `init` calls complete, so ordering between components is irrelevant.
    pub fn set_pin(&mut self, pin: PinId, value: PinValue) {
        self.outputs.pin_writes.push((pin, value));
    }

    /// Schedule `on_tick` to fire after `delay`. Call again from `on_tick`
    /// to repeat.
    pub fn schedule_tick(&mut self, delay: Duration) {
        self.outputs.tick_request = Some(delay);
    }

    /// The component this context belongs to.
    pub fn component_id(&self) -> ComponentId {
        self.component_id
    }

    /// Consume the context and return the collected outputs.
    pub(crate) fn take_outputs(self) -> PendingOutputs {
        self.outputs
    }
}

// ── RunCtx ────────────────────────────────────────────────────────────────────

/// Provided to all run-time `IcBehavior` methods. The IC expresses every
/// outgoing effect through this object; the event loop processes them
/// after the handler returns.
pub struct RunCtx<'a> {
    pub(crate) now: SimTime,
    pub(crate) outputs: &'a mut PendingOutputs,
}

impl<'a> RunCtx<'a> {
    pub(crate) fn new(now: SimTime, outputs: &'a mut PendingOutputs) -> Self {
        Self { now, outputs }
    }

    /// Drive an output pin. Queued for net resolution after the handler returns.
    pub fn set_pin(&mut self, pin: PinId, value: PinValue) {
        self.outputs.pin_writes.push((pin, value));
    }

    /// Initiate a bus transaction from this IC (master-side; rare in v1).
    pub fn bus_transmit(&mut self, bus: BusId, txn: OwnedBusTransaction) {
        self.outputs.bus_transmits.push((bus, txn));
    }

    /// Publish an MQTT message. `channel` is the manifest channel name;
    /// the event loop resolves it to the topic from the board YAML.
    pub fn mqtt_publish(&mut self, channel: &str, payload: MqttValue) {
        self.outputs.mqtt_publishes.push((channel.to_string(), payload));
    }

    /// Schedule `on_tick` to fire after `delay`. One-shot — call again
    /// from `on_tick` to repeat.
    pub fn schedule_tick(&mut self, delay: Duration) {
        self.outputs.tick_request = Some(delay);
    }

    /// Current virtual simulation time.
    pub fn now(&self) -> SimTime {
        self.now
    }
}
