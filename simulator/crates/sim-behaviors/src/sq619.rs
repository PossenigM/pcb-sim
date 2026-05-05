//! Servoflo SQ619 pressure sensor behavior.
//!
//! Read-only: 2-byte big-endian 15-bit value (bits 14..0).
//! Firmware: pressure_decimbar = (raw * 11581 + (-58740000) + 50000) / 100000
//! Inverse:  raw = (pressure_decimbar * 100000 + 58740000 - 50000) / 11581
//!
//! Default: 1013 hPa = 10130 decimbar → raw ≈ 5623.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

pub struct Sq619 {
    pressure_decimbar: f64,
}

impl Sq619 {
    pub fn new() -> Self {
        Self { pressure_decimbar: 10130.0 }
    }

    fn raw_value(&self) -> u16 {
        let raw = (self.pressure_decimbar * 100_000.0 + 58_740_000.0 - 50_000.0) / 11_581.0;
        (raw.round() as i32).clamp(0, 0x7FFF) as u16
    }
}

impl Default for Sq619 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Sq619 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if channel == "pressure" {
            if let MqttValue::Float(v) = payload {
                self.pressure_decimbar = v;
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
            BusTransaction::I2cRead { length, .. } => {
                if length < 2 {
                    return Ok(BusResponse::Data(vec![0u8; length]));
                }
                let raw = self.raw_value();
                Ok(BusResponse::Data(vec![
                    ((raw >> 8) & 0x7F) as u8,
                    (raw & 0xFF) as u8,
                ]))
            }
            // SQ619 is read-only; NACK writes.
            BusTransaction::I2cWrite { .. } => Ok(BusResponse::Nack),
            _ => Err(IcError::Unsupported("sq619: unsupported transaction")),
        }
    }
}
