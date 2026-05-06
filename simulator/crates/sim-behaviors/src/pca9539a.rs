//! NXP PCA9539A 16-bit I2C I/O expander behavior.
//!
//! Register map:
//!   0x00 In0   — input port 0  (RO, reflects pin states)
//!   0x01 In1   — input port 1  (RO, reflects pin states)
//!   0x02 Out0  — output port 0 (R/W)
//!   0x03 Out1  — output port 1 (R/W)
//!   0x04 Pol0  — polarity inversion 0 (R/W)
//!   0x05 Pol1  — polarity inversion 1 (R/W)
//!   0x06 Cfg0  — configuration 0, 1=input (R/W, default 0xFF)
//!   0x07 Cfg1  — configuration 1, 1=input (R/W, default 0xFF)

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, PinId, PinValue, RunCtx};

const NUM_REGS: usize = 8;
const NUM_PINS: usize = 16;

const REG_IN0: usize = 0;
const REG_IN1: usize = 1;
const REG_OUT0: usize = 2;
const REG_OUT1: usize = 3;
const REG_CFG0: usize = 6;
const REG_CFG1: usize = 7;

/// Pin names in order P0_0..P0_7, P1_0..P1_7.
const PIN_NAMES: [&str; NUM_PINS] = [
    "P0_0", "P0_1", "P0_2", "P0_3", "P0_4", "P0_5", "P0_6", "P0_7",
    "P1_0", "P1_1", "P1_2", "P1_3", "P1_4", "P1_5", "P1_6", "P1_7",
];

pub struct Pca9539a {
    regs: [u8; NUM_REGS],
    reg_ptr: u8,
    pin_ids: [Option<PinId>; NUM_PINS],
}

impl Pca9539a {
    pub fn new() -> Self {
        let mut regs = [0u8; NUM_REGS];
        regs[REG_CFG0] = 0xFF; // Cfg0: all inputs
        regs[REG_CFG1] = 0xFF; // Cfg1: all inputs
        Self {
            regs,
            reg_ptr: 0,
            pin_ids: [None; NUM_PINS],
        }
    }

    /// Returns true if pin `bit` (0..15) is configured as output (cfg bit == 0).
    fn is_output(&self, bit: usize) -> bool {
        let cfg = if bit < 8 { self.regs[REG_CFG0] } else { self.regs[REG_CFG1] };
        let shift = bit % 8;
        (cfg >> shift) & 1 == 0
    }

    /// Drive output pins based on the current Out0/Out1 and Cfg0/Cfg1 registers.
    /// Only drives pins whose configuration says "output".
    fn drive_outputs(&self, ctx: &mut RunCtx<'_>) {
        for bit in 0..NUM_PINS {
            if let Some(pin) = self.pin_ids[bit] {
                if self.is_output(bit) {
                    let reg = if bit < 8 { self.regs[REG_OUT0] } else { self.regs[REG_OUT1] };
                    let shift = bit % 8;
                    let value = if (reg >> shift) & 1 == 1 {
                        PinValue::High
                    } else {
                        PinValue::Low
                    };
                    ctx.set_pin(pin, value);
                } else {
                    // Input pin: release to high-Z (we don't drive it)
                    ctx.set_pin(pin, PinValue::HighZ);
                }
            }
        }
    }

    /// Write to registers (starting at reg_ptr), then drive any affected output pins.
    fn write_regs(&mut self, data: &[u8], ctx: &mut RunCtx<'_>) {
        if data.is_empty() {
            return;
        }
        self.reg_ptr = data[0];

        let mut output_changed = false;
        for (i, &byte) in data[1..].iter().enumerate() {
            let reg = self.reg_ptr as usize + i;
            // Input registers (0, 1) are read-only
            if reg >= 2 && reg < NUM_REGS {
                let old = self.regs[reg];
                self.regs[reg] = byte;
                // If output or config register changed, we need to update pins
                if old != byte && (reg == REG_OUT0 || reg == REG_OUT1 || reg == REG_CFG0 || reg == REG_CFG1) {
                    output_changed = true;
                }
            }
        }

        if output_changed {
            self.drive_outputs(ctx);
        }
    }
}

impl Default for Pca9539a {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pca9539a {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        for (i, name) in PIN_NAMES.iter().enumerate() {
            self.pin_ids[i] = ctx.pin_id(name);
        }

        // Apply fixed_inputs from board config: sets input register bits for
        // pins that are hardwired (e.g. board revision DIP switches).
        if let Some(map) = ctx.config_value("fixed_inputs").and_then(|v| v.as_map()) {
            for (pin_name, value_str) in map {
                let high = value_str.eq_ignore_ascii_case("HIGH");
                if let Some(bit) = PIN_NAMES.iter().position(|&n| n == pin_name) {
                    let reg = if bit < 8 { REG_IN0 } else { REG_IN1 };
                    let shift = bit % 8;
                    if high {
                        self.regs[reg] |= 1 << shift;
                    } else {
                        self.regs[reg] &= !(1 << shift);
                    }
                }
            }
        }

        Ok(())
    }

    fn on_pin_change(
        &mut self,
        pin: PinId,
        value: PinValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        // Update input registers for pins configured as inputs.
        for (bit, &pid) in self.pin_ids.iter().enumerate() {
            if pid == Some(pin) && !self.is_output(bit) {
                let high = matches!(value, PinValue::High);
                let reg = if bit < 8 { REG_IN0 } else { REG_IN1 };
                let shift = bit % 8;
                if high {
                    self.regs[reg] |= 1 << shift;
                } else {
                    self.regs[reg] &= !(1 << shift);
                }
                break;
            }
        }
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::I2cWrite { data, .. } => {
                self.write_regs(data, ctx);
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
                // If write has more than the pointer byte, apply those writes too
                if write.len() > 1 {
                    let mut full = vec![self.reg_ptr];
                    full.extend_from_slice(&write[1..]);
                    self.write_regs(&full, ctx);
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
