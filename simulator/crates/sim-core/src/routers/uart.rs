//! UART bus router.
//!
//! Point-to-point: exactly two peers per UART bus. Buffered both
//! directions (default 64 KiB per direction). When the buffer fills,
//! drop incoming bytes and emit a warning event.

use crate::types::{BusId, ComponentId};

pub const DEFAULT_UART_BUFFER_BYTES: usize = 64 * 1024;

pub struct UartRouter {
    pub bus_id: BusId,
    pub peer_a: ComponentId,
    pub peer_b: ComponentId,
    /// Bytes buffered for delivery to peer_a (sent by peer_b).
    pub buf_to_a: Vec<u8>,
    /// Bytes buffered for delivery to peer_b (sent by peer_a).
    pub buf_to_b: Vec<u8>,
    pub buffer_limit_bytes: usize,
}

impl UartRouter {
    // TODO: pub fn new(...) -> Self
    // TODO: pub fn forward(from: ComponentId, data: &[u8]) -> Result<(), Overflow>
    //   Buffer respecting the limit; on overflow, drop and emit warning.
}
