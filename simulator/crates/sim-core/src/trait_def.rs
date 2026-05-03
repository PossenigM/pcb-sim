//! The `IcBehavior` trait — every simulated peripheral implements this.

use crate::error::IcError;
use crate::time::SimTime;
use crate::types::{BusId, BusResponse, BusTransaction, MqttValue, PinId, PinValue};
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

/// Provided to `IcBehavior::init`. Exposes config and pin/bus name
/// resolution.
pub struct InitCtx<'a> {
    // TODO: fill in. Likely contains:
    //   - a reference to the resolved component config
    //   - a way to look up PinId by pin name local to this IC
    //   - a way to look up BusId by interface name local to this IC
    //   - a way to set initial pin values
    //   - a way to schedule the first tick
    _placeholder: std::marker::PhantomData<&'a ()>,
}

/// Provided to all run-time `IcBehavior` methods. The IC expresses every
/// outgoing effect through this object.
pub struct RunCtx<'a> {
    // TODO: fill in. Likely contains:
    //   - a queue of pending pin writes
    //   - a queue of pending MQTT publishes
    //   - a queue of pending master-side bus transmissions
    //   - the current SimTime
    //   - a logger handle
    _placeholder: std::marker::PhantomData<&'a ()>,
}

impl<'a> RunCtx<'a> {
    /// Drive an output pin. No-op if the value matches the current state.
    pub fn set_pin(&mut self, _pin: PinId, _value: PinValue) {
        // TODO: enqueue a pin-write event.
        unimplemented!()
    }

    /// Initiate a bus transaction (only meaningful for masters).
    pub fn bus_transmit(&mut self, _bus: BusId, _txn: BusTransaction<'_>) {
        // TODO: enqueue a master-initiated transaction.
        unimplemented!()
    }

    /// Publish an MQTT message. `channel` is the manifest channel name;
    /// the simulator resolves it to the topic from the board YAML.
    pub fn mqtt_publish(&mut self, _channel: &str, _payload: MqttValue) {
        // TODO: enqueue an MQTT publish.
        unimplemented!()
    }

    /// Schedule `on_tick` to fire after `delay`. One-shot — call again
    /// from `on_tick` to repeat.
    pub fn schedule_tick(&mut self, _delay: Duration) {
        // TODO: schedule a tick event.
        unimplemented!()
    }

    /// Current virtual time.
    pub fn now(&self) -> SimTime {
        // TODO: return the event loop's current SimTime.
        unimplemented!()
    }
}
