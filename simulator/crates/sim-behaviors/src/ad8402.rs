//! Analog Devices AD8402 dual-channel 256-position SPI digital potentiometer.
//!
//! SPI write frame (2 bytes, MSB-first):
//!   byte 0 = wiper value (0–255)
//!   byte 1 = channel address (0 or 1)
//!
//! Resistance = wiper / 255 × r_max  (r_max default 10 000 Ω)
//! Publishes ch0_resistance / ch1_resistance on every change.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

const DEFAULT_R_MAX: f64 = 10_000.0;

pub struct Ad8402 {
    wipers: [u8; 2],
    r_max: f64,
}

impl Ad8402 {
    pub fn new() -> Self {
        Self { wipers: [0; 2], r_max: DEFAULT_R_MAX }
    }

    fn resistance(&self, ch: usize) -> f64 {
        self.wipers[ch] as f64 / 255.0 * self.r_max
    }
}

impl Default for Ad8402 {
    fn default() -> Self { Self::new() }
}

impl IcBehavior for Ad8402 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::SpiTransfer { mosi } => {
                let resp = vec![0u8; mosi.len()];
                if mosi.len() >= 2 {
                    let wiper = mosi[0];
                    let ch = (mosi[1] & 0x01) as usize;
                    if self.wipers[ch] != wiper {
                        self.wipers[ch] = wiper;
                        let key = if ch == 0 { "ch0_resistance" } else { "ch1_resistance" };
                        ctx.mqtt_publish(key, MqttValue::Float(self.resistance(ch)));
                    }
                }
                Ok(BusResponse::Data(resp))
            }
            _ => Err(IcError::Unsupported("ad8402: unsupported transaction")),
        }
    }
}
