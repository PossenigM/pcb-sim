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
//! preserved. The adapter continues listening and accepts a new connection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde_bytes::ByteBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use sim_core::{
    event_loop::{EventSender, ExternalEvent, IpcOperation, IpcResponse},
    time::SimTime,
    types::{BusId, GpioDirection, GpioPull, PinId, PinValue},
    ComponentId,
};
use sim_protocol::{
    codec,
    codec::MAX_BODY_BYTES,
    messages::{
        AckResult, ClientMessage, ErrorCode, PinDirectionWire, PinPullWire, PinValueWire,
        ServerMessage,
    },
    version::PROTOCOL_VERSION,
};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("codec: {0}")]
    Codec(#[from] codec::CodecError),
    #[error("frame body too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("expected Hello as first message")]
    ExpectedHello,
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u32),
}

fn is_eof(e: &IpcError) -> bool {
    matches!(
        e,
        IpcError::Io(io) if matches!(
            io.kind(),
            std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
        )
    )
}

// ── Internal session types ────────────────────────────────────────────────────

/// Describes what kind of response to generate for a given internal request_id.
#[derive(Clone, Copy)]
enum RequestKind {
    I2cWrite      { wire_id: i64 },
    I2cRead       { wire_id: i64 },
    I2cWriteRead  { wire_id: i64 },
    SpiTransfer   { wire_id: i64 },
    GpioWrite     { wire_id: i64 },
    GpioRead      { wire_id: i64 },
    GpioConfigure { wire_id: i64 },
}

impl RequestKind {
    fn wire_id(self) -> i64 {
        match self {
            Self::I2cWrite { wire_id }
            | Self::I2cRead { wire_id }
            | Self::I2cWriteRead { wire_id }
            | Self::SpiTransfer { wire_id }
            | Self::GpioWrite { wire_id }
            | Self::GpioRead { wire_id }
            | Self::GpioConfigure { wire_id } => wire_id,
        }
    }
}

/// Commands sent from the read task to the write loop.
enum WriteCmd {
    /// Record this request_id, then dispatch it to the simulator event loop.
    Dispatch {
        request_id: u32,
        kind: RequestKind,
        sim_time: SimTime,
        operation: IpcOperation,
    },
    /// Send an error back immediately (e.g. unknown pin/bus name).
    SendNow(ServerMessage),
}

// ── Framing helpers ───────────────────────────────────────────────────────────

async fn read_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<Vec<u8>, IpcError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_BODY_BYTES {
        return Err(IpcError::FrameTooLarge(len));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    Ok(body)
}

/// Encode `msg` and write it as a length-prefixed frame.
async fn write_msg<W: AsyncWriteExt + Unpin>(w: &mut W, msg: &ServerMessage) -> Result<(), IpcError> {
    // codec::encode already prepends the 4-byte length prefix.
    let framed = codec::encode(msg)?;
    w.write_all(&framed).await?;
    Ok(())
}

// ── Value type conversions ────────────────────────────────────────────────────

fn pin_from_wire(v: PinValueWire) -> PinValue {
    match v {
        PinValueWire::Low        => PinValue::Low,
        PinValueWire::High       => PinValue::High,
        PinValueWire::Z          => PinValue::HighZ,
        PinValueWire::Analog(v)  => PinValue::Analog(v),
    }
}

fn pin_to_wire(v: PinValue) -> PinValueWire {
    match v {
        PinValue::Low        => PinValueWire::Low,
        PinValue::High       => PinValueWire::High,
        PinValue::HighZ      => PinValueWire::Z,
        PinValue::Analog(v)  => PinValueWire::Analog(v),
    }
}

fn dir_from_wire(d: PinDirectionWire) -> GpioDirection {
    match d {
        PinDirectionWire::In  => GpioDirection::In,
        PinDirectionWire::Out => GpioDirection::Out,
    }
}

fn pull_from_wire(p: Option<PinPullWire>) -> GpioPull {
    match p {
        None | Some(PinPullWire::None) => GpioPull::None,
        Some(PinPullWire::Up)          => GpioPull::Up,
        Some(PinPullWire::Down)        => GpioPull::Down,
    }
}

// ── Handshake ─────────────────────────────────────────────────────────────────

/// Read the first frame, expect Hello, send HelloAck. Returns client name.
async fn handshake<R, W>(r: &mut R, w: &mut W, mcu_id: &str) -> Result<String, IpcError>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let body = read_frame(r).await?;
    let msg: ClientMessage = codec::decode(&body)?;

    match msg {
        ClientMessage::Hello { protocol_version, client, .. } => {
            if protocol_version != PROTOCOL_VERSION {
                let _ = write_msg(w, &ServerMessage::Error {
                    id: None,
                    code: ErrorCode::UnsupportedVersion,
                    message: format!(
                        "expected version {PROTOCOL_VERSION}, got {protocol_version}"
                    ),
                    meta: None,
                }).await;
                return Err(IpcError::UnsupportedVersion(protocol_version));
            }
            write_msg(w, &ServerMessage::HelloAck {
                protocol_version: PROTOCOL_VERSION,
                mcu_id: mcu_id.to_owned(),
                meta: None,
            }).await?;
            Ok(client)
        }
        _ => {
            let _ = write_msg(w, &ServerMessage::Error {
                id: None,
                code: ErrorCode::BadRequest,
                message: "expected Hello as first message".into(),
                meta: None,
            }).await;
            Err(IpcError::ExpectedHello)
        }
    }
}

// ── ClientMessage → IpcOperation ─────────────────────────────────────────────

/// Translate an incoming client message into an IPC operation and the tracking
/// info needed to shape the response.
///
/// Returns `Ok((op, Some(kind)))` for request-response operations.
/// Returns `Ok((op, None))` for `UartTx` (fire-and-forget; no wire id).
/// Returns `Err(msg)` when name resolution fails; `msg` is sent immediately.
fn translate_client(
    msg: ClientMessage,
    pin_map: &HashMap<String, PinId>,
    bus_map: &HashMap<String, BusId>,
) -> Result<(IpcOperation, Option<RequestKind>), ServerMessage> {
    macro_rules! bus {
        ($name:expr, $id:expr) => {
            match bus_map.get(&$name) {
                Some(&b) => b,
                None => return Err(ServerMessage::Error {
                    id: Some($id),
                    code: ErrorCode::NoSuchBus,
                    message: format!("unknown bus: {}", $name),
                    meta: None,
                }),
            }
        };
    }
    macro_rules! pin {
        ($name:expr, $id:expr) => {
            match pin_map.get(&$name) {
                Some(&p) => p,
                None => return Err(ServerMessage::Error {
                    id: Some($id),
                    code: ErrorCode::NoSuchPin,
                    message: format!("unknown pin: {}", $name),
                    meta: None,
                }),
            }
        };
    }

    Ok(match msg {
        ClientMessage::Hello { .. } => {
            return Err(ServerMessage::Error {
                id: None,
                code: ErrorCode::BadRequest,
                message: "unexpected Hello after handshake".into(),
                meta: None,
            });
        }

        ClientMessage::I2cWrite { id, bus, address, data, .. } => (
            IpcOperation::I2cWrite { bus: bus!(bus, id), address, data: data.into_vec() },
            Some(RequestKind::I2cWrite { wire_id: id }),
        ),

        ClientMessage::I2cRead { id, bus, address, length, .. } => (
            IpcOperation::I2cRead { bus: bus!(bus, id), address, length: length as usize },
            Some(RequestKind::I2cRead { wire_id: id }),
        ),

        ClientMessage::I2cWriteRead { id, bus, address, write, read_length, .. } => (
            IpcOperation::I2cWriteRead {
                bus: bus!(bus, id),
                address,
                write: write.into_vec(),
                read_length: read_length as usize,
            },
            Some(RequestKind::I2cWriteRead { wire_id: id }),
        ),

        // The `cs` field identifies which slave is selected. Pass it through
        // so the event loop can use it when no GPIO-based CS is active.
        ClientMessage::SpiTransfer { id, bus, cs, mosi, .. } => (
            IpcOperation::SpiTransfer { bus: bus!(bus, id), cs: Some(cs), mosi: mosi.into_vec() },
            Some(RequestKind::SpiTransfer { wire_id: id }),
        ),

        ClientMessage::GpioWrite { id, pin, value, .. } => (
            IpcOperation::GpioWrite { pin: pin!(pin, id), value: pin_from_wire(value) },
            Some(RequestKind::GpioWrite { wire_id: id }),
        ),

        ClientMessage::GpioRead { id, pin, .. } => (
            IpcOperation::GpioRead { pin: pin!(pin, id) },
            Some(RequestKind::GpioRead { wire_id: id }),
        ),

        ClientMessage::GpioConfigure { id, pin, direction, pull, .. } => (
            IpcOperation::GpioConfigure {
                pin: pin!(pin, id),
                direction: dir_from_wire(direction),
                pull: pull_from_wire(pull),
            },
            Some(RequestKind::GpioConfigure { wire_id: id }),
        ),

        ClientMessage::UartTx { bus, data, .. } => {
            let bus_id = match bus_map.get(&bus) {
                Some(&b) => b,
                None => return Err(ServerMessage::Error {
                    id: None,
                    code: ErrorCode::NoSuchBus,
                    message: format!("unknown bus: {bus}"),
                    meta: None,
                }),
            };
            // Fire-and-forget: no wire id, no response sent to firmware.
            (IpcOperation::UartTx { bus: bus_id, data: data.into_vec() }, None)
        }
    })
}

// ── IpcResponse → ServerMessage ───────────────────────────────────────────────

fn translate_response(
    resp: IpcResponse,
    pending: &mut HashMap<u32, RequestKind>,
    pin_names: &HashMap<PinId, String>,
    bus_names: &HashMap<BusId, String>,
) -> Option<ServerMessage> {
    match resp {
        IpcResponse::Ack { request_id } => {
            let kind = pending.remove(&request_id)?;
            Some(match kind {
                RequestKind::I2cWrite { wire_id } =>
                    ServerMessage::I2cAck { id: wire_id, result: AckResult::Ok, meta: None },
                RequestKind::GpioWrite { wire_id } | RequestKind::GpioConfigure { wire_id } =>
                    ServerMessage::GpioAck { id: wire_id, result: AckResult::Ok, meta: None },
                other => {
                    warn!(request_id, "unexpected Ack for {:?} request; dropping",
                        std::mem::discriminant(&other));
                    return None;
                }
            })
        }

        IpcResponse::Nack { request_id } => {
            let kind = pending.remove(&request_id)?;
            Some(match kind {
                RequestKind::I2cWrite { wire_id } =>
                    ServerMessage::I2cAck { id: wire_id, result: AckResult::Nack, meta: None },
                RequestKind::I2cRead { wire_id } | RequestKind::I2cWriteRead { wire_id } =>
                    ServerMessage::I2cData { id: wire_id, result: AckResult::Nack, data: None, meta: None },
                RequestKind::SpiTransfer { wire_id } =>
                    ServerMessage::SpiData {
                        id: wire_id,
                        result: AckResult::Nack,
                        miso: ByteBuf::default(),
                        meta: None,
                    },
                RequestKind::GpioWrite { wire_id } | RequestKind::GpioConfigure { wire_id } =>
                    ServerMessage::GpioAck { id: wire_id, result: AckResult::Nack, meta: None },
                RequestKind::GpioRead { wire_id } =>
                    ServerMessage::GpioValue {
                        id: wire_id,
                        result: AckResult::Nack,
                        value: PinValueWire::Z,
                        meta: None,
                    },
            })
        }

        IpcResponse::Data { request_id, data } => {
            let kind = pending.remove(&request_id)?;
            Some(match kind {
                RequestKind::I2cRead { wire_id } | RequestKind::I2cWriteRead { wire_id } =>
                    ServerMessage::I2cData {
                        id: wire_id,
                        result: AckResult::Ok,
                        data: Some(ByteBuf::from(data)),
                        meta: None,
                    },
                RequestKind::SpiTransfer { wire_id } =>
                    ServerMessage::SpiData {
                        id: wire_id,
                        result: AckResult::Ok,
                        miso: ByteBuf::from(data),
                        meta: None,
                    },
                other => {
                    warn!(request_id, "unexpected Data for {:?} request; dropping",
                        std::mem::discriminant(&other));
                    return None;
                }
            })
        }

        IpcResponse::GpioValue { request_id, value } => {
            let kind = pending.remove(&request_id)?;
            match kind {
                RequestKind::GpioRead { wire_id } => Some(ServerMessage::GpioValue {
                    id: wire_id,
                    result: AckResult::Ok,
                    value: pin_to_wire(value),
                    meta: None,
                }),
                other => {
                    warn!(request_id, "unexpected GpioValue for {:?} request; dropping",
                        std::mem::discriminant(&other));
                    None
                }
            }
        }

        // Unsolicited events — no pending lookup needed.
        IpcResponse::GpioEvent { pin, value, sim_time } => {
            let pin_name = pin_names
                .get(&pin)
                .cloned()
                .unwrap_or_else(|| format!("pin#{}", pin.0));
            Some(ServerMessage::GpioEvent {
                pin: pin_name,
                value: pin_to_wire(value),
                sim_time_us: sim_time.as_micros() as u64,
                meta: None,
            })
        }

        IpcResponse::UartRx { bus, data, sim_time } => {
            let bus_name = bus_names
                .get(&bus)
                .cloned()
                .unwrap_or_else(|| format!("bus#{}", bus.0));
            Some(ServerMessage::UartRx {
                bus: bus_name,
                data: ByteBuf::from(data),
                sim_time_us: sim_time.as_micros() as u64,
                meta: None,
            })
        }

        IpcResponse::Error { request_id, message } => {
            let wire_id = pending.remove(&request_id).map(|k| k.wire_id());
            Some(ServerMessage::Error {
                id: wire_id,
                code: ErrorCode::InternalError,
                message,
                meta: None,
            })
        }
    }
}

// ── IpcAdapter ────────────────────────────────────────────────────────────────

/// Unix socket adapter for a single firmware-host component.
///
/// Call [`IpcAdapter::run`] inside a `tokio::spawn` alongside the event loop.
pub struct IpcAdapter {
    /// Human-readable component id (from board YAML), used as `mcu_id` in HelloAck.
    pub name: String,
    pub component: ComponentId,
    pub socket_path: PathBuf,
    /// Pin name → global PinId for this firmware host.
    pub pin_map: HashMap<String, PinId>,
    /// Bus interface name → global BusId for this firmware host.
    pub bus_map: HashMap<String, BusId>,
    pin_names: HashMap<PinId, String>,
    bus_names: HashMap<BusId, String>,
}

impl IpcAdapter {
    pub fn new(
        name: String,
        component: ComponentId,
        socket_path: PathBuf,
        pin_map: HashMap<String, PinId>,
        bus_map: HashMap<String, BusId>,
    ) -> Self {
        let pin_names = pin_map.iter().map(|(k, v)| (*v, k.clone())).collect();
        let bus_names = bus_map.iter().map(|(k, v)| (*v, k.clone())).collect();
        IpcAdapter { name, component, socket_path, pin_map, bus_map, pin_names, bus_names }
    }

    /// Run the accept loop. Blocks until `response_rx` is closed (event loop shutdown).
    ///
    /// Binds the socket, accepts firmware connections one at a time, and handles
    /// reconnection transparently: peripheral state is preserved in the event loop.
    pub async fn run(
        self,
        event_sender: EventSender,
        mut response_rx: mpsc::Receiver<IpcResponse>,
    ) -> anyhow::Result<()> {
        let _ = tokio::fs::remove_file(&self.socket_path).await;
        let listener = UnixListener::bind(&self.socket_path)?;
        info!(component = %self.name, path = ?self.socket_path, "IPC listening");

        let start = Instant::now();
        let adapter = Arc::new(self);

        loop {
            let (stream, _) = listener.accept().await?;
            info!(component = %adapter.name, "firmware connected");

            let (mut read_half, mut write_half) = stream.into_split();

            match handshake(&mut read_half, &mut write_half, &adapter.name).await {
                Ok(client) => debug!(component = %adapter.name, %client, "handshake ok"),
                Err(e) => {
                    warn!(component = %adapter.name, error = %e, "handshake failed; dropping");
                    continue;
                }
            }

            // Channel for the read task → write loop communication.
            let (write_tx, mut write_rx) = mpsc::channel::<WriteCmd>(256);

            // Spawn the read task.
            let sender = event_sender.clone();
            let adapter2 = Arc::clone(&adapter);
            let read_task = tokio::spawn(async move {
                let mut req_counter: u32 = 0;
                loop {
                    let body = match read_frame(&mut read_half).await {
                        Ok(b) => b,
                        Err(e) => {
                            if !is_eof(&e) {
                                error!(component = %adapter2.name, error = %e, "read error");
                            }
                            break;
                        }
                    };
                    let msg: ClientMessage = match codec::decode(&body) {
                        Ok(m) => m,
                        Err(e) => {
                            error!(component = %adapter2.name, error = %e, "decode error");
                            continue;
                        }
                    };
                    info!(component = %adapter2.name, message = ?msg, "socket rx");

                    let req_id = req_counter;
                    req_counter = req_counter.wrapping_add(1);
                    let sim_time = SimTime::from(start.elapsed());

                    match translate_client(msg, &adapter2.pin_map, &adapter2.bus_map) {
                        Ok((op, kind)) => {
                            if let Some(k) = kind {
                                if write_tx.send(WriteCmd::Dispatch {
                                    request_id: req_id,
                                    kind: k,
                                    sim_time,
                                    operation: op,
                                }).await.is_err() {
                                    break; // write loop exited
                                }
                            } else if !sender.send(ExternalEvent::IpcRequest {
                                request_id: req_id,
                                firmware_host: adapter2.component,
                                sim_time,
                                operation: op,
                            }).await {
                                break; // event loop gone
                            }
                        }
                        Err(err_msg) => {
                            // Name resolution failed: send error directly.
                            let _ = write_tx.send(WriteCmd::SendNow(err_msg)).await;
                        }
                    }
                }
                // write_tx is dropped here → write_rx.recv() returns None → session ends
            });

            // Write loop: process commands from read task and responses from event loop.
            let mut pending: HashMap<u32, RequestKind> = HashMap::new();
            'session: loop {
                tokio::select! {
                    biased;

                    cmd = write_rx.recv() => {
                        match cmd {
                            Some(WriteCmd::Dispatch { request_id, kind, sim_time, operation }) => {
                                pending.insert(request_id, kind);
                                if !event_sender.send(ExternalEvent::IpcRequest {
                                    request_id,
                                    firmware_host: adapter.component,
                                    sim_time,
                                    operation,
                                }).await {
                                    return Ok(()); // event loop gone
                                }
                            }
                            Some(WriteCmd::SendNow(msg)) => {
                                if write_msg(&mut write_half, &msg).await.is_err() {
                                    break 'session;
                                }
                            }
                            None => break 'session, // read task exited
                        }
                    }

                    resp = response_rx.recv() => {
                        match resp {
                            Some(r) => {
                                if let Some(msg) = translate_response(
                                    r,
                                    &mut pending,
                                    &adapter.pin_names,
                                    &adapter.bus_names,
                                ) {
                                    info!(component = %adapter.name, message = ?msg, "socket tx");
                                    if write_msg(&mut write_half, &msg).await.is_err() {
                                        break 'session;
                                    }
                                }
                            }
                            None => return Ok(()), // event loop shut down
                        }
                    }
                }
            }

            read_task.abort();
            info!(component = %adapter.name, "firmware disconnected; resuming accept loop");
        }
    }
}
