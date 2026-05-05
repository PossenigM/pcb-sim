//! Texas Instruments ADS7961 16-channel 10-bit SPI ADC behavior.
//!
//! Frame format (16-bit, MSB-first):
//!   MOSI bits 11..8 = channel select (manual mode, bit 13=1)
//!   MISO bits 15..12 = channel echoed, bits 11..2 = 10-bit result
//!
//! Full-scale = 2.5 V → raw 1023.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

pub struct Ads7961 {
    ch_volts: [f64; 16],
}

impl Ads7961 {
    pub fn new() -> Self {
        Self { ch_volts: [0.0; 16] }
    }

    fn volts_to_raw(v: f64) -> u16 {
        ((v / 2.5 * 1023.0).round() as i32).clamp(0, 1023) as u16
    }
}

impl Default for Ads7961 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Ads7961 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if let Some(num_str) = channel.strip_prefix("ch") {
            if let Ok(idx) = num_str.parse::<usize>() {
                if idx < 16 {
                    if let MqttValue::Float(v) = payload {
                        self.ch_volts[idx] = v;
                    }
                }
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
            BusTransaction::SpiTransfer { mosi } => {
                let mut resp = vec![0u8; mosi.len()];
                if mosi.len() >= 2 {
                    let word_in = ((mosi[0] as u16) << 8) | mosi[1] as u16;
                    // Bits 11..8 = channel select in manual mode.
                    let ch = ((word_in >> 8) & 0xF) as usize;
                    let raw = Self::volts_to_raw(self.ch_volts[ch]);
                    // Encode response: ch in bits 15..12, raw in bits 11..2.
                    let word_out: u16 = ((ch as u16 & 0xF) << 12) | ((raw & 0x3FF) << 2);
                    resp[0] = (word_out >> 8) as u8;
                    resp[1] = (word_out & 0xFF) as u8;
                }
                Ok(BusResponse::Data(resp))
            }
            _ => Err(IcError::Unsupported("ads7961: unsupported transaction")),
        }
    }
}
