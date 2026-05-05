//! Generic UART terminal stub behavior.
//!
//! Acts as the required second peer on a UART bus.  Discards all received
//! data and never initiates a transmission.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

pub struct UartTerminal;

impl UartTerminal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UartTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for UartTerminal {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::UartFrame { .. } => Ok(BusResponse::None),
            _ => Err(IcError::Unsupported("uart_terminal: not a UART bus")),
        }
    }
}
