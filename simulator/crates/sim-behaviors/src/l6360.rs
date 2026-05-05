//! ST L6360 IO-Link device transceiver behavior.
//!
//! Register map (I2C slave):
//!   0x00 STATUS  — power/fault flags (RO)
//!   0x01 CONFIG  — CQ mode
//!   0x02 CTRL1   — pulldown, overcurrent, debounce
//!   0x03 CTRL2   — IQ pulldown, L+ cutoff
//!   0x04 LED1_M  — LED1 MSB
//!   0x05 LED1_L  — LED1 LSB
//!   0x06 LED2_M  — LED2 MSB
//!   0x07 LED2_L  — LED2 LSB
//!   0x08 PARITY  — even parity byte (computed on read)
//!
//! Firmware block-writes [CONFIG..PARITY] (8 bytes starting at reg 0x01).
//! Firmware reads 2 bytes (STATUS, PARITY) with a plain I2C read.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

const NUM_REGS: usize = 9;

pub struct L6360 {
    regs: [u8; NUM_REGS],
}

impl L6360 {
    pub fn new() -> Self {
        let mut regs = [0u8; NUM_REGS];
        // STATUS bit 4 = L+ power OK — report "power good" at startup.
        regs[0] = 0x10;
        Self { regs }
    }

    fn compute_parity(regs: &[u8; NUM_REGS]) -> u8 {
        // Even parity: XOR of each register byte CONFIG..LED2_L
        let mut p = 0u8;
        for r in &regs[1..8] {
            p ^= r;
        }
        p
    }
}

impl Default for L6360 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for L6360 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            // Plain read: firmware reads STATUS + PARITY (2 bytes).
            BusTransaction::I2cRead { length, .. } => {
                self.regs[8] = Self::compute_parity(&self.regs);
                let n = length.min(2);
                Ok(BusResponse::Data(self.regs[..n].to_vec()))
            }

            // Block write starting at a register address (first data byte = reg).
            BusTransaction::I2cWrite { data, .. } => {
                if data.is_empty() {
                    return Ok(BusResponse::None);
                }
                let start = data[0] as usize;
                for (i, &byte) in data[1..].iter().enumerate() {
                    let reg = start + i;
                    // Only writable registers (skip read-only STATUS=0).
                    if reg >= 1 && reg < NUM_REGS {
                        self.regs[reg] = byte;
                    }
                }
                Ok(BusResponse::None)
            }

            // Write-read: set register pointer, then read.
            BusTransaction::I2cWriteRead { write, read_length, .. } => {
                let start = write.first().copied().unwrap_or(0) as usize;
                self.regs[8] = Self::compute_parity(&self.regs);
                let available = NUM_REGS.saturating_sub(start);
                let n = read_length.min(available);
                Ok(BusResponse::Data(self.regs[start..start + n].to_vec()))
            }

            _ => Err(IcError::Unsupported("l6360: unsupported transaction")),
        }
    }
}
