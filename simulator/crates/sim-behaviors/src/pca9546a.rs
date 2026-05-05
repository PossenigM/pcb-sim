//! NXP PCA9546A 4-channel I2C multiplexer behavior.
//!
//! Control register (1 byte): bit N enables channel N.
//! Write 0x00 to disable all channels.
//! Read returns the current control register.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

pub struct Pca9546a {
    control: u8,
}

impl Pca9546a {
    pub fn new() -> Self {
        Self { control: 0x00 }
    }
}

impl Default for Pca9546a {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pca9546a {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::I2cWrite { data, .. } => {
                if let Some(&b) = data.first() {
                    self.control = b;
                }
                Ok(BusResponse::None)
            }
            BusTransaction::I2cRead { length, .. } => {
                let n = length.min(1);
                Ok(BusResponse::Data(vec![self.control; n]))
            }
            _ => Err(IcError::Unsupported("pca9546a: unsupported transaction")),
        }
    }
}
