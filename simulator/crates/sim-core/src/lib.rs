//! `sim-core` — the heart of the simulator.
//!
//! Defines the `IcBehavior` trait, the supporting types, the event loop,
//! and the bus and net routers. No I/O happens in this crate — that is
//! the responsibility of `sim-mqtt`, `sim-ipc`, and `sim-bin`.

pub mod error;
pub mod event_loop;
pub mod routers;
pub mod time;
pub mod trait_def;
pub mod types;

pub use error::{IcError, SimError};
pub use trait_def::{IcBehavior, InitCtx, PendingOutputs, RunCtx};
pub use types::{
    BusId, BusResponse, BusTransaction, ComponentId, ConfigValue,
    GpioDirection, GpioPull, MqttValue, OwnedBusTransaction, PinId, PinValue,
};
pub use time::SimTime;
pub use event_loop::{
    EventLoop, EventLoopConfig, EventLoopHandles, EventSender,
    ExternalEvent, IpcOperation, IpcResponse, MqttPublish,
};
