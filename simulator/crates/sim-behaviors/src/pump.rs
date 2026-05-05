//! Generic pump actuator behavior.
//!
//! Two input pins:
//!   - PWM:    drives the pump speed signal
//!   - ENABLE: gate pin from the IO expander
//!
//! The pump publishes `state: true` when ENABLE is HIGH and PWM is HIGH.
//! When ENABLE goes LOW, the pump publishes `state: false` regardless of PWM.

use sim_core::{IcBehavior, IcError, InitCtx, MqttValue, PinId, PinValue, RunCtx};

pub struct Pump {
    pwm_pin: Option<PinId>,
    enable_pin: Option<PinId>,
    pwm_high: bool,
    enable_high: bool,
    last_published: Option<bool>,
}

impl Pump {
    pub fn new() -> Self {
        Self {
            pwm_pin: None,
            enable_pin: None,
            pwm_high: false,
            enable_high: false,
            last_published: None,
        }
    }

    fn effective_state(&self) -> bool {
        self.enable_high && self.pwm_high
    }
}

impl Default for Pump {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pump {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        self.pwm_pin = ctx.pin_id("PWM");
        self.enable_pin = ctx.pin_id("ENABLE");
        Ok(())
    }

    fn on_pin_change(
        &mut self,
        pin: PinId,
        value: PinValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        let high = matches!(value, PinValue::High);

        if Some(pin) == self.pwm_pin {
            self.pwm_high = high;
        } else if Some(pin) == self.enable_pin {
            self.enable_high = high;
        } else {
            return Ok(());
        }

        let state = self.effective_state();
        if self.last_published != Some(state) {
            self.last_published = Some(state);
            ctx.mqtt_publish("state", MqttValue::Bool(state));
        }
        Ok(())
    }
}
