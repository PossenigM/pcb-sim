//! `sim-ipc` — Unix socket adapter for firmware connections.
//!
//! Listens on the path declared in each `firmware_host` component's
//! `firmware:` block. When firmware connects:
//!
//!   1. Performs the `hello` handshake (see `docs/wire-protocol.md` §5).
//!   2. Reads framed MessagePack messages from the socket.
//!   3. Translates each `ClientMessage` to an event for the simulator
//!      event loop.
//!   4. Receives responses and events from the event loop and writes
//!      them back as `ServerMessage` frames.
//!
//! Reconnection: when firmware disconnects, peripheral state is
//! preserved (see ADR / open question in architecture.md). The adapter
//! continues listening and accepts a new connection.

use sim_core::ComponentId;
use std::path::PathBuf;

pub struct IpcAdapter {
    pub component: ComponentId,
    pub socket_path: PathBuf,
    // TODO: tokio Listener, current connection handle, write half, etc.
}

impl IpcAdapter {
    // TODO: pub async fn new(component, socket_path) -> Self
    // TODO: pub async fn run(...) -> the accept-loop entry point.
    //   For each accepted connection:
    //     - read hello, validate version, send hello_ack
    //     - spawn a read task: parse incoming messages, push events
    //     - spawn a write task: serialize outgoing responses/events
    //     - on disconnect, log and resume accept-loop

    // TODO: helper functions to convert wire types ↔ sim-core types:
    //   ClientMessage::I2cWrite { bus, address, data, .. }
    //     → sim-core::Event::IpcRequest(...) with a BusTransaction
    //   sim-core pin change → ServerMessage::GpioEvent
    //   sim-core uart byte stream → ServerMessage::UartRx (chunked)
    //   etc.
}

// TODO: framing helpers.
//   async fn read_frame(read: &mut impl AsyncRead) -> Result<Vec<u8>>
//   async fn write_frame(write: &mut impl AsyncWrite, frame: &[u8]) -> Result<()>
//   These read/write the 4-byte LE length prefix + body. See the
//   `sim-protocol::codec` module for sync helpers; the async versions
//   live here.
