//! `sim-config` — load YAML files (manifests, board definitions),
//! validate against the JSON Schemas, produce strongly-typed Rust
//! structs ready for the simulator core.

pub mod board;
pub mod library;
pub mod manifest;
pub mod validate;

pub use board::Board;
pub use library::IcLibrary;
pub use manifest::IcManifest;
