//! NXP PCA9546A 4-channel I2C multiplexer behavior.
//!
//! Control register (1 byte): bit N enables channel N.
//! Write 0x00 to disable all channels.
//! Read returns the current control register.
//!
//! The mux forwards I2C transactions from the upstream bus to enabled
//! downstream channels. When a write to the control register changes the
//! enabled channels, the behavior emits `bus_transmit` on the newly-active
//! downstream bus(es) for any subsequent upstream traffic. In v1 the event
//! loop does not route IC-initiated I2C, so downstream forwarding is a
//! no-op until the event loop gains mux-aware routing.

use sim_core::{BusId, BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, RunCtx};

const NUM_CHANNELS: usize = 4;

const CHANNEL_INTERFACES: [&str; NUM_CHANNELS] = [
    "i2c_ch0", "i2c_ch1", "i2c_ch2", "i2c_ch3",
];

pub struct Pca9546a {
    control: u8,
    /// Downstream bus IDs resolved from interface names.
    downstream_buses: [Option<BusId>; NUM_CHANNELS],
}

impl Pca9546a {
    pub fn new() -> Self {
        Self {
            control: 0x00,
            downstream_buses: [None; NUM_CHANNELS],
        }
    }

    /// Returns the list of currently enabled downstream BusIds.
    pub fn enabled_channels(&self) -> Vec<BusId> {
        (0..NUM_CHANNELS)
            .filter(|&ch| (self.control >> ch) & 1 == 1)
            .filter_map(|ch| self.downstream_buses[ch])
            .collect()
    }
}

impl Default for Pca9546a {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Pca9546a {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        for (ch, name) in CHANNEL_INTERFACES.iter().enumerate() {
            self.downstream_buses[ch] = ctx.bus_id(name);
        }
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
                    self.control = b & 0x0F; // Only lower 4 bits are valid
                }
                Ok(BusResponse::None)
            }
            BusTransaction::I2cRead { length, .. } => {
                let n = length.min(1);
                Ok(BusResponse::Data(vec![self.control; n]))
            }
            BusTransaction::I2cWriteRead { write, read_length, .. } => {
                if let Some(&b) = write.first() {
                    self.control = b & 0x0F;
                }
                let n = read_length.min(1);
                Ok(BusResponse::Data(vec![self.control; n]))
            }
            _ => Err(IcError::Unsupported("pca9546a: unsupported transaction")),
        }
    }
}
