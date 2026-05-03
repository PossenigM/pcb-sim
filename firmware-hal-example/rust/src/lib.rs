//! Reference Rust HAL for pcb-sim.
//!
//! Blocking, single-threaded API. For applications that need async,
//! wrap the calls in spawn_blocking or write a tokio variant.
//!
//! # Example
//!
//! ```no_run
//! use pcb_sim_hal::{Hal, PinValue};
//! let mut hal = Hal::connect("/tmp/board_sim/mcu1.sock").unwrap();
//! hal.gpio_write("PA0", PinValue::High).unwrap();
//! ```

use std::os::unix::net::UnixStream;
use std::path::Path;
use thiserror::Error;

// Read/Write traits will be needed once the framing helpers are implemented.
// They are intentionally not imported here yet to avoid unused-import warnings.

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PinValue {
    Low,
    High,
    Z,
}

#[derive(Debug, Error)]
pub enum HalError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("encode failed: {0}")]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("decode failed: {0}")]
    Decode(#[from] rmp_serde::decode::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("nack from device")]
    Nack,
}

pub struct Hal {
    stream: UnixStream,
    next_id: i64,
    // TODO: queue of pending events received while waiting for a response
}

impl Hal {
    pub fn connect(_path: impl AsRef<Path>) -> Result<Self, HalError> {
        // TODO:
        //   1. UnixStream::connect(path)
        //   2. send Hello { protocol_version: 1, client: "pcb_sim-hal-rust" }
        //   3. receive HelloAck, verify protocol_version
        //   4. return self
        unimplemented!()
    }

    pub fn gpio_write(&mut self, _pin: &str, _value: PinValue) -> Result<(), HalError> {
        // TODO: send GpioWrite, await GpioAck.
        unimplemented!()
    }

    pub fn gpio_read(&mut self, _pin: &str) -> Result<PinValue, HalError> {
        // TODO: send GpioRead, await GpioValue.
        unimplemented!()
    }

    pub fn i2c_write(&mut self, _bus: &str, _addr: u8, _data: &[u8]) -> Result<(), HalError> {
        // TODO
        unimplemented!()
    }

    pub fn i2c_read(&mut self, _bus: &str, _addr: u8, _len: usize) -> Result<Vec<u8>, HalError> {
        // TODO
        unimplemented!()
    }

    pub fn i2c_write_read(
        &mut self,
        _bus: &str,
        _addr: u8,
        _write: &[u8],
        _read_len: usize,
    ) -> Result<Vec<u8>, HalError> {
        // TODO
        unimplemented!()
    }

    /// Poll for an unsolicited event with a timeout. Returns Ok(None) if
    /// no event arrived in time.
    pub fn poll_event(
        &mut self,
        _timeout: std::time::Duration,
    ) -> Result<Option<Event>, HalError> {
        // TODO:
        //   - First drain self.event_queue.
        //   - Otherwise set socket read timeout, attempt to read a frame.
        //   - Classify as response (queue or discard) vs event (return).
        unimplemented!()
    }

    // ----- helpers -----

    fn fresh_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // TODO: fn write_frame(&mut self, msg: &impl Serialize) -> Result<(), HalError>
    // TODO: fn read_frame(&mut self) -> Result<Vec<u8>, HalError>
    // TODO: fn await_response(&mut self, id: i64) -> Result<ServerMessage, HalError>
}

#[derive(Debug, Clone)]
pub enum Event {
    Gpio { pin: String, value: PinValue, sim_time_us: u64 },
    UartRx { bus: String, data: Vec<u8>, sim_time_us: u64 },
    UartOverflow { bus: String, dropped_bytes: u32, sim_time_us: u64 },
    Sim { kind: String, message: Option<String>, sim_time_us: u64 },
}
