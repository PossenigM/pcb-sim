//! UART bus router.
//!
//! Point-to-point: exactly two peers per UART bus. Buffered both
//! directions (default 64 KiB per direction). When the buffer fills,
//! bytes are dropped and `ForwardResult::overflow_bytes` is non-zero.
//!
//! Typical flow:
//!   1. Sender calls `forward(from, data)` — appends to the dest buffer,
//!      returns dest `ComponentId` and any overflow count.
//!   2. Caller calls `drain_for_peer(dest)` to take the buffered bytes and
//!      deliver them (via `on_bus_transaction` or IPC `UartRx`).

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

/// Result of a `forward()` call.
pub struct ForwardResult {
    /// The destination peer.
    pub dest: ComponentId,
    /// Number of bytes dropped due to buffer overflow (0 if no overflow).
    pub overflow_bytes: usize,
}

impl UartRouter {
    pub fn new(
        bus_id: BusId,
        peer_a: ComponentId,
        peer_b: ComponentId,
        buffer_limit_bytes: usize,
    ) -> Self {
        Self {
            bus_id,
            peer_a,
            peer_b,
            buf_to_a: Vec::new(),
            buf_to_b: Vec::new(),
            buffer_limit_bytes,
        }
    }

    /// Buffer `data` for delivery to the peer opposite of `from`.
    /// If the destination buffer is full, excess bytes are dropped.
    pub fn forward(&mut self, from: ComponentId, data: &[u8]) -> ForwardResult {
        let (dest, buf) = if from == self.peer_a {
            (self.peer_b, &mut self.buf_to_b)
        } else {
            (self.peer_a, &mut self.buf_to_a)
        };

        let available = self.buffer_limit_bytes.saturating_sub(buf.len());
        let to_add = data.len().min(available);
        buf.extend_from_slice(&data[..to_add]);
        let overflow_bytes = data.len() - to_add;

        if overflow_bytes > 0 {
            tracing::warn!(
                bus_id = ?self.bus_id,
                overflow_bytes,
                "UART buffer overflow: bytes dropped"
            );
        }

        ForwardResult { dest, overflow_bytes }
    }

    /// Take all buffered bytes destined for `peer` (drains the buffer).
    pub fn drain_for_peer(&mut self, peer: ComponentId) -> Vec<u8> {
        if peer == self.peer_a {
            std::mem::take(&mut self.buf_to_a)
        } else {
            std::mem::take(&mut self.buf_to_b)
        }
    }
}
