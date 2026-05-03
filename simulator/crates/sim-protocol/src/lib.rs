//! `sim-protocol` — wire types for the firmware ↔ simulator IPC protocol.
//!
//! See `docs/wire-protocol.md` for the full specification. This crate
//! defines the message structs and the length-prefixed framing codec.

pub mod codec;
pub mod messages;
pub mod version;

pub use codec::{decode, encode, CodecError};
pub use messages::*;
pub use version::PROTOCOL_VERSION;
