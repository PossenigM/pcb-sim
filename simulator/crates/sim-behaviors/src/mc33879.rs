//! NXP MC33879 8-channel SPI output driver behavior.
//!
//! 2-byte SPI frame:
//!   tx[0] = open-load detect mask
//!   tx[1] = output enable mask (1 = channel on)
//!   rx[0] = 0x00 (unused)
//!   rx[1] = fault status of *previous* frame (0x00 = no faults in simulation)

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

pub struct Mc33879 {
    output_mask: u8,
}

impl Mc33879 {
    pub fn new() -> Self {
        Self { output_mask: 0 }
    }
}

impl Default for Mc33879 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Mc33879 {
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
                let mut resp = vec![0u8; mosi.len()];
                if mosi.len() >= 2 {
                    let new_outputs = mosi[1];
                    if new_outputs != self.output_mask {
                        self.output_mask = new_outputs;
                        ctx.mqtt_publish("outputs", MqttValue::Int(new_outputs as i64));
                    }
                    // rx[1] = fault status (always 0x00 — no simulated faults)
                    resp[1] = 0x00;
                }
                Ok(BusResponse::Data(resp))
            }
            _ => Err(IcError::Unsupported("mc33879: unsupported transaction")),
        }
    }
}
