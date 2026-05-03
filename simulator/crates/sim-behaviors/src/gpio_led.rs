//! Generic GPIO LED behavior.
//!
//! Listens on its single input pin. Whenever the pin transitions, publishes
//! the new boolean state to MQTT channel `state`.
//!
//! Manifest reference: `ic-library/generic_gpio_led/manifest.yaml`.

use sim_core::{IcBehavior, IcError, InitCtx, MqttValue, PinId, PinValue, RunCtx};

pub struct GpioLed {
    /// Cached last state. Used to suppress duplicate publishes.
    last_state: Option<bool>,
}

impl GpioLed {
    pub fn new() -> Self {
        Self { last_state: None }
    }
}

impl Default for GpioLed {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for GpioLed {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        // TODO: nothing to do at init beyond accepting the default pin state.
        //       Could optionally publish an initial `state: false` here.
        Ok(())
    }

    fn on_pin_change(
        &mut self,
        _pin: PinId,
        value: PinValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        let state = match value {
            PinValue::High => true,
            PinValue::Low => false,
            // HighZ: leave state unchanged. Real LEDs would be off, but
            // most boards have a pull resistor; not modeling that here.
            PinValue::HighZ => return Ok(()),
        };
        if self.last_state == Some(state) {
            return Ok(());
        }
        self.last_state = Some(state);
        ctx.mqtt_publish("state", MqttValue::Bool(state));
        Ok(())
    }
}
