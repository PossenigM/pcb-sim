//! Broadcom AFBR-710SMZ SFP transceiver module behavior.
//!
//! Dual-address I2C slave:
//!   0x50 — Serial ID / configuration registers (SFF-8472 Table 4-1)
//!   0x51 — Diagnostic monitoring (DOM) registers
//!
//! Simulates a healthy, present module. Responds to register reads with
//! plausible defaults. The four control/status signals (SFP_MOD, RX_LOSS,
//! TX_FAULT, TX_DISABLE) are exposed as pins and wired to the PCA9539A
//! IO expander in the board YAML.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

/// 256-byte register spaces for each I2C address.
pub struct Afbr710Smz {
    regs_a0: [u8; 256], // 0x50 registers (serial ID)
    regs_a2: [u8; 256], // 0x51 registers (DOM)
    reg_ptr: u8,
}

impl Afbr710Smz {
    pub fn new() -> Self {
        let mut regs_a0 = [0u8; 256];
        // SFF-8472 mandatory fields for valid module identification:
        // Byte 0: Identifier = 0x03 (SFP)
        regs_a0[0] = 0x03;
        // Byte 1: Ext. Identifier = 0x04 (MOD_DEF definition)
        regs_a0[1] = 0x04;
        // Byte 2: Connector = 0x07 (LC)
        regs_a0[2] = 0x07;
        // Byte 6: Transceiver code = 0x10 (1000BASE-SX)
        regs_a0[6] = 0x10;
        // Byte 12: Nominal BR = 0x0D (1300 Mbaud / 100)
        regs_a0[12] = 0x0D;
        // Byte 92: Enhanced options = 0x60 (DOM + alarms implemented)
        regs_a0[92] = 0x60;

        let regs_a2 = [0u8; 256];

        Self {
            regs_a0,
            regs_a2,
            reg_ptr: 0,
        }
    }
}

impl Default for Afbr710Smz {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Afbr710Smz {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::I2cWrite { address, data, .. } => {
                if data.is_empty() {
                    return Ok(BusResponse::None);
                }
                self.reg_ptr = data[0];
                let regs = match address {
                    0x50 => &mut self.regs_a0,
                    0x51 => &mut self.regs_a2,
                    _ => return Ok(BusResponse::Nack),
                };
                // Write remaining bytes starting at register pointer
                for (i, &byte) in data[1..].iter().enumerate() {
                    let idx = (self.reg_ptr as usize + i) & 0xFF;
                    regs[idx] = byte;
                }
                Ok(BusResponse::None)
            }

            BusTransaction::I2cRead { address, length, .. } => {
                let regs = match address {
                    0x50 => &self.regs_a0,
                    0x51 => &self.regs_a2,
                    _ => return Ok(BusResponse::Nack),
                };
                let start = self.reg_ptr as usize;
                let mut out = Vec::with_capacity(length);
                for i in 0..length {
                    out.push(regs[(start + i) & 0xFF]);
                }
                Ok(BusResponse::Data(out))
            }

            BusTransaction::I2cWriteRead { address, write, read_length, .. } => {
                if let Some(&ptr) = write.first() {
                    self.reg_ptr = ptr;
                }
                let regs = match address {
                    0x50 => &self.regs_a0,
                    0x51 => &self.regs_a2,
                    _ => return Ok(BusResponse::Nack),
                };
                let start = self.reg_ptr as usize;
                let mut out = Vec::with_capacity(read_length);
                for i in 0..read_length {
                    out.push(regs[(start + i) & 0xFF]);
                }
                Ok(BusResponse::Data(out))
            }

            _ => Err(IcError::Unsupported("afbr710smz: unsupported transaction")),
        }
    }
}
