//! Texas Instruments LM73 digital temperature sensor behavior.
//!
//! Firmware: write reg ptr 0x00, read 2 bytes big-endian.
//! Encoding: raw / 128 = temperature_°C.
//! Default: 25.0 °C → raw 0x0C80.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

pub struct Lm73 {
    temperature_c: f64,
    reg_ptr: u8,
}

impl Lm73 {
    pub fn new() -> Self {
        Self { temperature_c: 25.0, reg_ptr: 0 }
    }

    fn raw_temp(&self) -> u16 {
        ((self.temperature_c * 128.0).round() as i32).max(0) as u16
    }

    fn temp_bytes(&self) -> Vec<u8> {
        let raw = self.raw_temp();
        vec![(raw >> 8) as u8, (raw & 0xFF) as u8]
    }
}

impl Default for Lm73 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Lm73 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if channel == "temperature" {
            if let MqttValue::Float(v) = payload {
                self.temperature_c = v;
            }
        }
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::I2cWrite { data, .. } => {
                if let Some(&ptr) = data.first() {
                    self.reg_ptr = ptr;
                }
                Ok(BusResponse::None)
            }
            BusTransaction::I2cRead { .. } => {
                Ok(BusResponse::Data(self.temp_bytes()))
            }
            BusTransaction::I2cWriteRead { write, .. } => {
                if let Some(&ptr) = write.first() {
                    self.reg_ptr = ptr;
                }
                Ok(BusResponse::Data(self.temp_bytes()))
            }
            _ => Err(IcError::Unsupported("lm73: unsupported transaction")),
        }
    }
}
