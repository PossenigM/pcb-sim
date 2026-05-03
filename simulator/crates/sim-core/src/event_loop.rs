//! The simulator's event loop.
//!
//! Single-threaded priority queue ordered by `SimTime`. I/O happens on
//! separate async tasks (in `sim-mqtt` and `sim-ipc`); they enqueue
//! events here.
//!
//! Event sources:
//!   1. IPC adapter: firmware sent a request.
//!   2. MQTT subscriber: broker delivered a message.
//!   3. Tick scheduler: an IC asked to be ticked at a specific time.
//!   4. Net cascade: one IC's pin write triggered another IC's
//!      `on_pin_change`.

// TODO: Define the `Event` enum (one variant per source above).
// TODO: Implement the priority queue (BinaryHeap<Reverse<(SimTime, Event)>>).
// TODO: Implement the dispatch loop:
//   - pop next event
//   - advance current SimTime
//   - dispatch to the appropriate router or behavior
//   - drain RunCtx outputs and enqueue resulting events
// TODO: Provide a public `EventLoop` type with:
//   - `new()`
//   - `enqueue(event)` for I/O tasks to push events in
//   - `run()` to drive the loop until shutdown
//   - `shutdown()` for graceful termination
