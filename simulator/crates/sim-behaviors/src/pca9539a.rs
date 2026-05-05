//! NXP PCA9539A 16-bit I2C I/O expander behavior.
//!
//! Register map:
//!   0x00 In0   — input port 0  (RO, returns 0x00)
//!   0x01 In1   — input port 1  (RO, returns 0x00)
//!   0x02 Out0  — output port 0 (R/W)
//!   0x03 Out1  — output port 1 (R/W)
//!   0x04 Pol0  — polarity inversion 0 (R/W)
//!   0x05 Pol1  — polarity inversion 1 (R/W)
//!   0x06 Cfg0  — configuration 0, 1=input (R/W, default 0xFF)
//!   0x07 Cfg1  — configuration 1, 1=input (R/W, default 0xFF)

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

const NUM_REGS: usize = 8;

pub struct Pca9539a {
    regs: [u8; NUM_REGS],
    reg_ptr: u8,
}

impl Pca9539a {
    pub fn new() -> Self {
        let mut regs = [0u8; NUM_REGS];
        regs[6] = 0xFF; // Cfg0: all inputs
        regs[7] = 0xFF; // Cfg1: all inputs
        Self { regs, reg_ptr: 0 }
    }
}

impl Default for Pca9539a {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pca9539a {
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
                if data.is_empty() {
                    return Ok(BusResponse::None);
                }
                self.reg_ptr = data[0];
                for (i, &byte) in data[1..].iter().enumerate() {
                    let reg = self.reg_ptr as usize + i;
                    if reg >= 2 && reg < NUM_REGS {
                        self.regs[reg] = byte;
                    }
                }
                Ok(BusResponse::None)
            }

            BusTransaction::I2cRead { length, .. } => {
                let start = self.reg_ptr as usize;
                let available = NUM_REGS.saturating_sub(start);
                let n = length.min(available);
                Ok(BusResponse::Data(self.regs[start..start + n].to_vec()))
            }

            BusTransaction::I2cWriteRead { write, read_length, .. } => {
                if let Some(&ptr) = write.first() {
                    self.reg_ptr = ptr;
                }
                let start = self.reg_ptr as usize;
                let available = NUM_REGS.saturating_sub(start);
                let n = read_length.min(available);
                Ok(BusResponse::Data(self.regs[start..start + n].to_vec()))
            }

            _ => Err(IcError::Unsupported("pca9539a: unsupported transaction")),
        }
    }
}
