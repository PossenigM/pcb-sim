//! Generic heater actuator behavior.
//!
//! Input pin:
//!   - PWM: receives the duty-cycle value (0–255) as `PinValue::Analog(duty)`
//!          from the MCU's FPGA register write.
//!
//! MQTT publish: `pwm` (int, 0–255) — consumed by the physics simulation.

use sim_core::{IcBehavior, IcError, InitCtx, MqttValue, PinId, PinValue, RunCtx};

pub struct Heater {
    pwm_pin: Option<PinId>,
    last_duty: u32,
}

impl Heater {
    pub fn new() -> Self {
        Self { pwm_pin: None, last_duty: 0 }
    }
}

impl Default for Heater {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Heater {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        self.pwm_pin = ctx.pin_id("PWM");
        Ok(())
    }

    fn on_pin_change(
        &mut self,
        pin: PinId,
        value: PinValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if Some(pin) != self.pwm_pin {
            return Ok(());
        }
        let duty = match value {
            PinValue::Analog(v) => v,
            PinValue::High => 255,
            PinValue::Low | PinValue::HighZ => 0,
        };
        if duty != self.last_duty {
            self.last_duty = duty;
            ctx.mqtt_publish("pwm", MqttValue::Int(duty as i64));
        }
        Ok(())
    }
}
