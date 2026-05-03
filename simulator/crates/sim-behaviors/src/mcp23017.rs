//! Microchip MCP23017 behavior.
//!
//! 16-bit I2C GPIO expander. Two banks (A, B) of 8 pins each.
//! Firmware reads/writes its register map over I2C; the IC drives or
//! reads its GPIO pins accordingly.
//!
//! Manifest reference: `ic-library/microchip_mcp23017/manifest.yaml`.
//!
//! Datasheet: MCP23017 Table 1-3 (register map). Key registers (BANK=0):
//!   0x00 IODIRA   — direction (1=input, 0=output) for bank A
//!   0x01 IODIRB   — direction for bank B
//!   0x12 GPIOA    — read input or write output for bank A
//!   0x13 GPIOB    — read input or write output for bank B
//!   0x14 OLATA    — output latch A (writes here also reflect on GPIOA)
//!   0x15 OLATB    — output latch B
//! Plus: GPPUA/B, IPOLA/B, GPINTENA/B, INTFA/B, INTCAPA/B, etc.

use sim_core::{
    BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx,
};

pub struct Mcp23017 {
    /// 22-byte register file (BANK=0 layout).
    pub registers: [u8; 22],
    pub register_pointer: Option<u8>,
}

impl Mcp23017 {
    pub fn new() -> Self {
        let mut regs = [0u8; 22];
        // After reset, IODIR is all-1 (all pins are inputs).
        regs[0x00] = 0xFF;
        regs[0x01] = 0xFF;
        Self {
            registers: regs,
            register_pointer: None,
        }
    }
}

impl Default for Mcp23017 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Mcp23017 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        // TODO: assert default pin states (HighZ for inputs, etc.).
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        // TODO: implement register read/write logic.
        //   - Writing to GPIOA/OLATA → for each bit configured as output
        //     in IODIRA, drive the corresponding GPA pin via ctx.set_pin.
        //   - Same for bank B.
        //   - Reading GPIOA → return current state of bank A pins
        //     (combination of input pin readings + latched outputs).
        //   - Writing IODIRA/B → may need to release driven pins (set to
        //     HighZ) for any pins switched to input.
        let _ = txn;
        Err(IcError::Unsupported("mcp23017 i2c handling not yet implemented"))
    }

    fn on_pin_change(
        &mut self,
        _pin: sim_core::PinId,
        _value: sim_core::PinValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        // TODO: when an input pin changes, update GPIOA/B register bits.
        //       If GPINTEN says the pin is interrupt-enabled, update INTF
        //       and drive the corresponding INTA/INTB output pin.
        Ok(())
    }
}
