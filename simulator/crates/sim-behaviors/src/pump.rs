//! Generic pump actuator behavior.
//!
//! Input pins:
//!   - PWM:    receives the duty-cycle value (0–255) as `PinValue::Analog(duty)`
//!             from the MCU's FPGA register write.
//!   - ENABLE: gate signal from the IO expander; when LOW the effective duty
//!             published to MQTT is 0 regardless of the PWM value.
//!
//! Output pin:
//!   - TACHO: driven with `PinValue::Analog(rpm)` so the MCU can read the
//!            current pump speed via a register read.
//!
//! MQTT publish: `pwm` (int, 0–255) — consumed by the physics simulation.
//! MQTT subscribe: `tacho` (int, RPM) — feedback from the physics simulation.

use sim_core::{IcBehavior, IcError, InitCtx, MqttValue, PinId, PinValue, RunCtx};

pub struct Pump {
    pwm_pin: Option<PinId>,
    enable_pin: Option<PinId>,
    tacho_pin: Option<PinId>,
    pwm_duty: u32,
    enabled: bool,
    last_published: u32,
}

impl Pump {
    pub fn new() -> Self {
        Self {
            pwm_pin: None,
            enable_pin: None,
            tacho_pin: None,
            pwm_duty: 0,
            enabled: false,
            last_published: 0,
        }
    }

    fn effective_duty(&self) -> u32 {
        if self.enabled { self.pwm_duty } else { 0 }
    }

    fn publish_if_changed(&mut self, ctx: &mut RunCtx<'_>) {
        let duty = self.effective_duty();
        if duty != self.last_published {
            self.last_published = duty;
            ctx.mqtt_publish("pwm", MqttValue::Int(duty as i64));
        }
    }
}

impl Default for Pump {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pump {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        self.pwm_pin    = ctx.pin_id("PWM");
        self.enable_pin = ctx.pin_id("ENABLE");
        self.tacho_pin  = ctx.pin_id("TACHO");
        if let Some(tacho) = self.tacho_pin {
            ctx.set_pin(tacho, PinValue::Analog(0));
        }
        Ok(())
    }

    fn on_pin_change(
        &mut self,
        pin: PinId,
        value: PinValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if Some(pin) == self.pwm_pin {
            self.pwm_duty = match value {
                PinValue::Analog(v) => v,
                PinValue::High => 255,
                PinValue::Low | PinValue::HighZ => 0,
            };
        } else if Some(pin) == self.enable_pin {
            self.enabled = matches!(value, PinValue::High | PinValue::Analog(_));
        } else {
            return Ok(());
        }
        self.publish_if_changed(ctx);
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if channel != "tacho" {
            return Ok(());
        }
        let rpm = match payload {
            MqttValue::Int(v) => v.max(0) as u32,
            MqttValue::Float(v) => v.max(0.0) as u32,
            _ => return Ok(()),
        };
        if let Some(tacho_pin) = self.tacho_pin {
            ctx.set_pin(tacho_pin, PinValue::Analog(rpm));
        }
        Ok(())
    }
}
