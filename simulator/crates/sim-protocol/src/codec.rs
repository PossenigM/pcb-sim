//! Length-prefixed MessagePack framing.
//!
//! Wire format: `[u32 LE length][MessagePack body]`. See
//! `docs/wire-protocol.md` §2.

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// Maximum allowed message body size (16 MiB). Larger messages are a
/// protocol error.
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("message too large: {0} bytes (max {})", MAX_BODY_BYTES)]
    TooLarge(usize),

    #[error("encode failed: {0}")]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("decode failed: {0}")]
    Decode(#[from] rmp_serde::decode::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Serialize `msg` and write it as a length-prefixed MessagePack frame.
///
/// Returns the framed bytes (length prefix + body) ready to write to a
/// socket.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, CodecError> {
    let body = rmp_serde::to_vec_named(msg)?;
    if body.len() > MAX_BODY_BYTES {
        return Err(CodecError::TooLarge(body.len()));
    }
    let mut framed = Vec::with_capacity(4 + body.len());
    framed.extend_from_slice(&(body.len() as u32).to_le_bytes());
    framed.extend_from_slice(&body);
    Ok(framed)
}

/// Decode a single MessagePack body (length prefix already stripped) into
/// a typed message.
pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, CodecError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(CodecError::TooLarge(body.len()));
    }
    Ok(rmp_serde::from_slice(body)?)
}

// TODO: Async reader helper that reads exactly 4 bytes for the length,
//       then exactly N bytes for the body, and returns a `Vec<u8>` ready
//       for `decode()`. Probably belongs in `sim-ipc` since it depends on
//       a tokio AsyncRead trait, but a sync version could live here for
//       use in tests.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{ClientMessage, PinValueWire};

    #[test]
    fn round_trip_gpio_write() {
        let msg = ClientMessage::GpioWrite {
            id: 42,
            pin: "PA0".into(),
            value: PinValueWire::High,
            meta: None,
        };
        let framed = encode(&msg).unwrap();
        // Strip 4-byte length prefix
        let body = &framed[4..];
        let decoded: ClientMessage = decode(body).unwrap();
        match decoded {
            ClientMessage::GpioWrite { id, pin, value, .. } => {
                assert_eq!(id, 42);
                assert_eq!(pin, "PA0");
                assert_eq!(value, PinValueWire::High);
            }
            _ => panic!("wrong variant"),
        }
    }
}
