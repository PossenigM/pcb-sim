//! The simulator's event loop.
//!
//! Single-threaded priority queue ordered by `SimTime`. I/O happens on
//! separate async tasks (in `sim-mqtt` and `sim-ipc`); they enqueue
//! events here via `EventSender`.
//!
//! # Design
//!
//! `EventLoop` owns all component behaviors, all bus routers, and all nets.
//! External async tasks communicate with it through two channels:
//!
//! - **Inbound** (`ExternalEvent`): IPC requests from firmware and MQTT
//!   messages from the broker.
//! - **Outbound** (`IpcResponse` / `MqttPublish`): responses and publishes
//!   sent back to the respective adapters.
//!
//! # Time
//!
//! `SimTime` is virtual and monotonic. It advances when IPC requests arrive
//! (the IPC adapter stamps each request with the current wall-clock elapsed
//! time). Ticks are scheduled relative to `SimTime` and fire once the event
//! loop's `now` advances past their target.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use tokio::sync::mpsc;

use crate::error::SimError;
use crate::routers::{
    i2c::I2cRouter,
    net::Net,
    spi::SpiRouter,
    uart::UartRouter,
};
use crate::time::SimTime;
use crate::trait_def::{IcBehavior, InitCtx, PendingOutputs, RunCtx};
use crate::types::{
    BusId, BusResponse, ComponentId, ConfigValue, GpioDirection, GpioPull, MqttValue,
    OwnedBusTransaction, PinId, PinValue,
};

// ── Channel capacity ──────────────────────────────────────────────────────────

const CHANNEL_CAPACITY: usize = 1024;

// ── Public event / message types ─────────────────────────────────────────────

/// Events sent into the event loop by the IPC and MQTT adapters.
pub enum ExternalEvent {
    /// A firmware request arrived on the IPC socket.
    IpcRequest {
        request_id: u32,
        /// Which firmware-host component issued the request.
        firmware_host: ComponentId,
        /// Simulation time at which this request was stamped (by the IPC adapter).
        sim_time: SimTime,
        operation: IpcOperation,
    },
    /// An MQTT message arrived from the broker.
    MqttMessage {
        component: ComponentId,
        /// Manifest-declared channel name (not the raw topic).
        channel: String,
        payload: MqttValue,
    },
    /// Graceful shutdown signal.
    Shutdown,
}

/// Firmware-initiated operations, expressed in sim-core terms.
/// The IPC adapter translates from `sim-protocol` wire messages to these.
pub enum IpcOperation {
    I2cWrite    { bus: BusId, address: u8, data: Vec<u8> },
    I2cRead     { bus: BusId, address: u8, length: usize },
    I2cWriteRead { bus: BusId, address: u8, write: Vec<u8>, read_length: usize },
    SpiTransfer { bus: BusId, mosi: Vec<u8> },
    GpioWrite   { pin: PinId, value: PinValue },
    GpioRead    { pin: PinId },
    GpioConfigure { pin: PinId, direction: GpioDirection, pull: GpioPull },
    UartTx      { bus: BusId, data: Vec<u8> },
}

/// Responses and unsolicited events sent back to the IPC adapter.
pub enum IpcResponse {
    Ack         { request_id: u32 },
    Data        { request_id: u32, data: Vec<u8> },
    Nack        { request_id: u32 },
    GpioValue   { request_id: u32, value: PinValue },
    /// Unsolicited: a GPIO input pin changed state (firmware subscribed via configure).
    GpioEvent   { pin: PinId, value: PinValue, sim_time: SimTime },
    /// Unsolicited: data received from the UART peer.
    UartRx      { bus: BusId, data: Vec<u8>, sim_time: SimTime },
    Error       { request_id: u32, message: String },
}

/// An MQTT publish queued by a behavior, to be sent by the MQTT adapter.
pub struct MqttPublish {
    /// Fully-resolved topic (prefix + relative topic from board YAML).
    pub topic: String,
    pub payload: MqttValue,
}

// ── EventSender ───────────────────────────────────────────────────────────────

/// Cloneable handle for pushing events into the event loop.
/// Give one clone to the IPC adapter and one to the MQTT adapter.
#[derive(Clone)]
pub struct EventSender(mpsc::Sender<ExternalEvent>);

impl EventSender {
    pub async fn send(&self, event: ExternalEvent) -> bool {
        self.0.send(event).await.is_ok()
    }

    pub fn try_send(&self, event: ExternalEvent) -> bool {
        self.0.try_send(event).is_ok()
    }
}

// ── EventLoopConfig ───────────────────────────────────────────────────────────

/// Everything `EventLoop::new` needs to set up the simulation world.
/// Built by `sim-bin` from the loaded `Board` + `IcLibrary`.
pub struct EventLoopConfig {
    /// Peripheral behaviors, keyed by component ID.
    pub behaviors: HashMap<ComponentId, Box<dyn IcBehavior>>,
    /// Per-component: pin name → global PinId (for InitCtx).
    pub pin_maps: HashMap<ComponentId, HashMap<String, PinId>>,
    /// Per-component: interface name → global BusId (for InitCtx).
    pub bus_maps: HashMap<ComponentId, HashMap<String, BusId>>,
    /// Per-component: config key → value (for InitCtx).
    pub configs: HashMap<ComponentId, HashMap<String, ConfigValue>>,
    /// One router per I2C bus.
    pub i2c_routers: Vec<I2cRouter>,
    /// One router per SPI bus.
    pub spi_routers: Vec<SpiRouter>,
    /// One router per UART bus.
    pub uart_routers: Vec<UartRouter>,
    /// All GPIO nets.
    pub nets: Vec<Net>,
    /// Maps a SPI master's CS PinId → (BusId, CS pin name on master).
    /// Built from the board's SPI slave declarations.
    pub cs_pin_to_bus: HashMap<PinId, (BusId, String)>,
    /// Per-component: channel name → fully-resolved MQTT topic.
    /// Built from ComponentMqtt.publish + board MQTT prefix.
    pub publish_routes: HashMap<ComponentId, HashMap<String, String>>,
    /// Component IDs that are firmware hosts (no behavior; uses IPC).
    pub firmware_hosts: HashSet<ComponentId>,
}

/// Handles returned alongside the `EventLoop` so the adapter tasks can
/// communicate with it.
pub struct EventLoopHandles {
    /// Push `ExternalEvent`s into the loop. Clone for each adapter.
    pub event_sender: EventSender,
    /// Receive `IpcResponse`s produced by the loop (one per firmware host).
    pub ipc_response_rx: mpsc::Receiver<IpcResponse>,
    /// Receive `MqttPublish`es produced by behaviors.
    pub mqtt_publish_rx: mpsc::Receiver<MqttPublish>,
}

// ── EventLoop ─────────────────────────────────────────────────────────────────

pub struct EventLoop {
    now: SimTime,

    behaviors: HashMap<ComponentId, Box<dyn IcBehavior>>,
    i2c_routers: HashMap<BusId, I2cRouter>,
    spi_routers: HashMap<BusId, SpiRouter>,
    uart_routers: HashMap<BusId, UartRouter>,
    nets: Vec<Net>,

    /// PinId → (net index, endpoint index within that net).
    pin_to_net_endpoint: HashMap<PinId, (usize, usize)>,
    /// Current driven value for pins that are NOT part of any net
    /// (e.g. firmware GPIO not wired anywhere, or bus-owned pins).
    pin_states: HashMap<PinId, PinValue>,
    /// SPI CS pin → (bus, CS pin name on master).
    cs_pin_to_bus: HashMap<PinId, (BusId, String)>,
    /// Currently active (asserted low) CS pin name per SPI bus.
    spi_active_cs: HashMap<BusId, String>,

    tick_heap: BinaryHeap<Reverse<(SimTime, ComponentId)>>,

    external_rx: mpsc::Receiver<ExternalEvent>,
    ipc_tx: mpsc::Sender<IpcResponse>,
    mqtt_tx: mpsc::Sender<MqttPublish>,

    publish_routes: HashMap<ComponentId, HashMap<String, String>>,
    firmware_hosts: HashSet<ComponentId>,
}

impl EventLoop {
    /// Construct the event loop, call `init` on every behavior, and return
    /// the loop alongside channel handles for the adapter tasks.
    pub fn new(mut cfg: EventLoopConfig) -> Result<(Self, EventLoopHandles), SimError> {
        let (event_tx, external_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (ipc_tx, ipc_response_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (mqtt_tx, mqtt_publish_rx) = mpsc::channel(CHANNEL_CAPACITY);

        // Index routers by BusId.
        let i2c_routers: HashMap<BusId, I2cRouter> =
            cfg.i2c_routers.drain(..).map(|r| (r.bus_id, r)).collect();
        let spi_routers: HashMap<BusId, SpiRouter> =
            cfg.spi_routers.drain(..).map(|r| (r.bus_id, r)).collect();
        let uart_routers: HashMap<BusId, UartRouter> =
            cfg.uart_routers.drain(..).map(|r| (r.bus_id, r)).collect();

        // Build pin → net lookup.
        let mut pin_to_net_endpoint: HashMap<PinId, (usize, usize)> = HashMap::new();
        for (net_idx, net) in cfg.nets.iter().enumerate() {
            for (ep_idx, ep) in net.endpoints.iter().enumerate() {
                pin_to_net_endpoint.insert(ep.pin, (net_idx, ep_idx));
            }
        }

        let mut el = EventLoop {
            now: SimTime::ZERO,
            behaviors: HashMap::new(),
            i2c_routers,
            spi_routers,
            uart_routers,
            nets: cfg.nets,
            pin_to_net_endpoint,
            pin_states: HashMap::new(),
            cs_pin_to_bus: cfg.cs_pin_to_bus,
            spi_active_cs: HashMap::new(),
            tick_heap: BinaryHeap::new(),
            external_rx,
            ipc_tx,
            mqtt_tx,
            publish_routes: cfg.publish_routes,
            firmware_hosts: cfg.firmware_hosts,
        };

        // ── Init each behavior (pass 1: call init, collect outputs) ──────────
        let mut deferred_outputs: Vec<(ComponentId, PendingOutputs)> = Vec::new();
        let empty_pins: HashMap<String, PinId> = HashMap::new();
        let empty_buses: HashMap<String, BusId> = HashMap::new();
        let empty_config: HashMap<String, ConfigValue> = HashMap::new();

        for (comp_id, mut behavior) in cfg.behaviors {
            let pin_map = cfg.pin_maps.get(&comp_id).unwrap_or(&empty_pins);
            let bus_map = cfg.bus_maps.get(&comp_id).unwrap_or(&empty_buses);
            let config = cfg.configs.get(&comp_id).unwrap_or(&empty_config);

            let mut ctx = InitCtx::new(comp_id, pin_map, bus_map, config);
            behavior.init(&mut ctx).map_err(SimError::Ic)?;
            let outputs = ctx.take_outputs();

            el.behaviors.insert(comp_id, behavior);
            deferred_outputs.push((comp_id, outputs));
        }

        // ── Process init outputs (pass 2: after all behaviors are ready) ──────
        for (comp_id, outputs) in deferred_outputs {
            if let Some(delay) = outputs.tick_request {
                el.tick_heap.push(Reverse((SimTime::ZERO + delay, comp_id)));
            }
            // Initial pin writes go through the normal output processor,
            // which drives nets and cascades on_pin_change.
            let pin_only = PendingOutputs {
                pin_writes: outputs.pin_writes,
                ..PendingOutputs::default()
            };
            el.process_outputs(comp_id, pin_only);
        }

        let handles = EventLoopHandles {
            event_sender: EventSender(event_tx),
            ipc_response_rx,
            mqtt_publish_rx,
        };

        Ok((el, handles))
    }

    /// Drive the event loop until all `EventSender`s are dropped or a
    /// `Shutdown` event is received.
    pub async fn run(&mut self) {
        loop {
            // Fire any ticks that are now due.
            self.fire_due_ticks();

            // Drain all immediately available external events.
            let mut got_any = false;
            loop {
                match self.external_rx.try_recv() {
                    Ok(ev) => {
                        got_any = true;
                        if !self.handle_external(ev) {
                            return;
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => return,
                }
            }

            if got_any {
                // Re-check ticks; simtime may have advanced.
                continue;
            }

            // Nothing immediately available — wait for the next event.
            match self.external_rx.recv().await {
                Some(ev) => {
                    if !self.handle_external(ev) {
                        return;
                    }
                }
                None => return, // all senders dropped
            }
        }
    }

    // ── Internal dispatch ─────────────────────────────────────────────────────

    fn fire_due_ticks(&mut self) {
        while let Some(Reverse((at, _))) = self.tick_heap.peek() {
            if *at > self.now {
                break;
            }
            let Reverse((at, comp)) = self.tick_heap.pop().unwrap();
            let mut outputs = PendingOutputs::default();
            if let Some(behavior) = self.behaviors.get_mut(&comp) {
                let mut ctx = RunCtx::new(at, &mut outputs);
                if let Err(e) = behavior.on_tick(at, &mut ctx) {
                    tracing::error!(component = ?comp, error = %e, "on_tick failed");
                }
            }
            self.process_outputs(comp, outputs);
        }
    }

    /// Returns `false` if the loop should stop.
    fn handle_external(&mut self, ev: ExternalEvent) -> bool {
        match ev {
            ExternalEvent::Shutdown => return false,

            ExternalEvent::IpcRequest { request_id, firmware_host, sim_time, operation } => {
                if sim_time > self.now {
                    self.now = sim_time;
                }
                self.handle_ipc(request_id, firmware_host, operation);
                // Ticks may now be due after time advanced.
                self.fire_due_ticks();
            }

            ExternalEvent::MqttMessage { component, channel, payload } => {
                let mut outputs = PendingOutputs::default();
                if let Some(behavior) = self.behaviors.get_mut(&component) {
                    let mut ctx = RunCtx::new(self.now, &mut outputs);
                    if let Err(e) = behavior.on_mqtt_message(&channel, payload, &mut ctx) {
                        tracing::error!(component = ?component, error = %e, "on_mqtt_message failed");
                    }
                }
                self.process_outputs(component, outputs);
            }
        }
        true
    }

    fn handle_ipc(&mut self, request_id: u32, firmware_host: ComponentId, op: IpcOperation) {
        match op {
            IpcOperation::I2cWrite { bus, address, data } => {
                let txn = OwnedBusTransaction::I2cWrite { address, data };
                let resp = self.dispatch_i2c(bus, &txn);
                let msg = match resp {
                    BusResponse::None | BusResponse::Data(_) => IpcResponse::Ack { request_id },
                    BusResponse::Nack => IpcResponse::Nack { request_id },
                };
                let _ = self.ipc_tx.try_send(msg);
            }

            IpcOperation::I2cRead { bus, address, length } => {
                let txn = OwnedBusTransaction::I2cRead { address, length };
                let resp = self.dispatch_i2c(bus, &txn);
                let msg = match resp {
                    BusResponse::Data(bytes) => IpcResponse::Data { request_id, data: bytes },
                    _ => IpcResponse::Nack { request_id },
                };
                let _ = self.ipc_tx.try_send(msg);
            }

            IpcOperation::I2cWriteRead { bus, address, write, read_length } => {
                let txn = OwnedBusTransaction::I2cWriteRead { address, write, read_length };
                let resp = self.dispatch_i2c(bus, &txn);
                let msg = match resp {
                    BusResponse::Data(bytes) => IpcResponse::Data { request_id, data: bytes },
                    _ => IpcResponse::Nack { request_id },
                };
                let _ = self.ipc_tx.try_send(msg);
            }

            IpcOperation::SpiTransfer { bus, mosi } => {
                // Look up which slave is selected via the currently-active CS pin.
                let slave_id = self.spi_active_cs.get(&bus).cloned().and_then(|cs| {
                    self.spi_routers.get(&bus).and_then(|r| r.route_cs(&cs))
                });

                let msg = match slave_id {
                    None => {
                        tracing::warn!(bus = ?bus, "SPI transfer but no CS pin is asserted");
                        IpcResponse::Nack { request_id }
                    }
                    Some(slave) => {
                        let txn = OwnedBusTransaction::SpiTransfer { mosi };
                        let mut outputs = PendingOutputs::default();
                        let resp = match self.behaviors.get_mut(&slave) {
                            Some(b) => {
                                let mut ctx = RunCtx::new(self.now, &mut outputs);
                                b.on_bus_transaction(txn.as_transaction(), &mut ctx)
                                    .unwrap_or_else(|e| {
                                        tracing::error!(error = %e, "on_bus_transaction (SPI) failed");
                                        BusResponse::Nack
                                    })
                            }
                            None => BusResponse::Nack,
                        };
                        self.process_outputs(slave, outputs);
                        match resp {
                            BusResponse::Data(bytes) => IpcResponse::Data { request_id, data: bytes },
                            _ => IpcResponse::Nack { request_id },
                        }
                    }
                };
                let _ = self.ipc_tx.try_send(msg);
            }

            IpcOperation::GpioWrite { pin, value } => {
                self.drive_gpio(firmware_host, pin, value);
                let _ = self.ipc_tx.try_send(IpcResponse::Ack { request_id });
            }

            IpcOperation::GpioRead { pin } => {
                let value = self.read_gpio(pin);
                let _ = self.ipc_tx.try_send(IpcResponse::GpioValue { request_id, value });
            }

            IpcOperation::GpioConfigure { pin, direction, pull } => {
                // In v1, pin direction is fixed by the manifest. We store the
                // firmware's runtime config for reference but don't propagate it
                // to the net's endpoint direction.
                tracing::debug!(
                    pin = ?pin,
                    direction = ?direction,
                    pull = ?pull,
                    "GpioConfigure (v1: direction is manifest-driven, runtime config stored only)"
                );
                let _ = self.ipc_tx.try_send(IpcResponse::Ack { request_id });
            }

            IpcOperation::UartTx { bus, data } => {
                self.handle_uart_tx(firmware_host, bus, &data);
                let _ = self.ipc_tx.try_send(IpcResponse::Ack { request_id });
            }
        }
    }

    // ── I2C dispatch ──────────────────────────────────────────────────────────

    fn dispatch_i2c(&mut self, bus: BusId, txn: &OwnedBusTransaction) -> BusResponse {
        // Route (drops the borrow on i2c_routers before we touch behaviors).
        let slave_id = match self.i2c_routers.get(&bus) {
            Some(router) => router.route(txn),
            None => {
                tracing::warn!(bus = ?bus, "I2C transaction on unknown bus");
                return BusResponse::Nack;
            }
        };

        let slave_id = match slave_id {
            Some(id) => id,
            None => return BusResponse::Nack,
        };

        let mut outputs = PendingOutputs::default();
        let response = match self.behaviors.get_mut(&slave_id) {
            Some(behavior) => {
                let mut ctx = RunCtx::new(self.now, &mut outputs);
                behavior
                    .on_bus_transaction(txn.as_transaction(), &mut ctx)
                    .unwrap_or_else(|e| {
                        tracing::error!(component = ?slave_id, error = %e, "on_bus_transaction (I2C) failed");
                        BusResponse::Nack
                    })
            }
            None => BusResponse::Nack,
        };

        self.process_outputs(slave_id, outputs);
        response
    }

    // ── GPIO ──────────────────────────────────────────────────────────────────

    fn drive_gpio(&mut self, from: ComponentId, pin: PinId, value: PinValue) {
        // Update SPI CS state if this is a CS pin.
        if let Some((bus, cs_name)) = self.cs_pin_to_bus.get(&pin).cloned() {
            match value {
                PinValue::Low => {
                    self.spi_active_cs.insert(bus, cs_name);
                }
                _ => {
                    self.spi_active_cs.remove(&bus);
                }
            }
        }

        // Drive through the net router or update local state.
        let outputs = PendingOutputs {
            pin_writes: vec![(pin, value)],
            ..PendingOutputs::default()
        };
        self.process_outputs(from, outputs);
    }

    fn read_gpio(&self, pin: PinId) -> PinValue {
        if let Some(&(net_idx, _)) = self.pin_to_net_endpoint.get(&pin) {
            self.nets[net_idx].resolved
        } else {
            self.pin_states.get(&pin).copied().unwrap_or(PinValue::HighZ)
        }
    }

    // ── UART ─────────────────────────────────────────────────────────────────

    fn handle_uart_tx(&mut self, from: ComponentId, bus: BusId, data: &[u8]) {
        // Forward into the router buffer, capturing dest before dropping the borrow.
        let (dest, overflow_bytes) = match self.uart_routers.get_mut(&bus) {
            Some(router) => {
                let result = router.forward(from, data);
                let dest = result.dest;
                let buffered = router.drain_for_peer(dest);
                let overflow = result.overflow_bytes;
                // Deliver buffered bytes to the destination.
                if self.firmware_hosts.contains(&dest) {
                    let _ = self.ipc_tx.try_send(IpcResponse::UartRx {
                        bus,
                        data: buffered,
                        sim_time: self.now,
                    });
                } else {
                    let mut outputs = PendingOutputs::default();
                    if let Some(beh) = self.behaviors.get_mut(&dest) {
                        let txn = OwnedBusTransaction::UartFrame { data: buffered };
                        let mut ctx = RunCtx::new(self.now, &mut outputs);
                        let _ = beh.on_bus_transaction(txn.as_transaction(), &mut ctx);
                    }
                    self.process_outputs(dest, outputs);
                }
                (dest, overflow)
            }
            None => {
                tracing::warn!(bus = ?bus, "UART Tx on unknown bus");
                return;
            }
        };

        if overflow_bytes > 0 {
            tracing::warn!(
                bus = ?bus, dest = ?dest, overflow_bytes,
                "UART buffer overflow: bytes dropped"
            );
        }
    }

    // ── Output processing ─────────────────────────────────────────────────────

    /// Drain `PendingOutputs` collected from a behavior call:
    /// - MQTT publishes → sent to the MQTT adapter.
    /// - Tick request → pushed to the heap.
    /// - Bus transmits → routed (IC-initiated, rare in v1).
    /// - Pin writes → drive nets, cascade `on_pin_change` iteratively.
    fn process_outputs(&mut self, from: ComponentId, mut outputs: PendingOutputs) {
        // MQTT publishes.
        for (channel, payload) in outputs.mqtt_publishes.drain(..) {
            match self
                .publish_routes
                .get(&from)
                .and_then(|m| m.get(&channel))
                .cloned()
            {
                Some(topic) => {
                    let _ = self.mqtt_tx.try_send(MqttPublish { topic, payload });
                }
                None => {
                    tracing::debug!(
                        component = ?from,
                        channel = %channel,
                        "behavior published to channel with no topic mapping; ignored"
                    );
                }
            }
        }

        // Tick scheduling.
        if let Some(delay) = outputs.tick_request.take() {
            self.tick_heap.push(Reverse((self.now + delay, from)));
        }

        // IC-initiated bus transmits (master-side; unusual in v1).
        for (bus, txn) in outputs.bus_transmits.drain(..) {
            match &txn {
                OwnedBusTransaction::UartFrame { data } => {
                    self.handle_uart_tx(from, bus, &data.clone());
                }
                _ => {
                    // I2C/SPI master transactions from IC behaviors: future enhancement.
                    tracing::debug!(
                        component = ?from, bus = ?bus,
                        "IC-initiated I2C/SPI bus_transmit is not routed in v1"
                    );
                }
            }
        }

        // Pin writes — processed iteratively to handle net cascades without recursion.
        let mut cascade: Vec<(PinId, PinValue)> = outputs.pin_writes.drain(..).collect();

        while let Some((pin, value)) = cascade.pop() {
            if let Some(&(net_idx, ep_idx)) = self.pin_to_net_endpoint.get(&pin) {
                // Drive the net and collect notification targets (copy out of borrow).
                let result = self.nets[net_idx].drive(ep_idx, value);

                if result.changed {
                    let new_val = result.new_value;

                    // Collect (component, pin) pairs to notify (all copied — no borrow held).
                    let to_notify: Vec<(ComponentId, PinId)> = result
                        .endpoints_to_notify
                        .iter()
                        .map(|&i| {
                            let ep = &self.nets[net_idx].endpoints[i];
                            (ep.component, ep.pin)
                        })
                        .collect();

                    for (comp, notif_pin) in to_notify {
                        if self.firmware_hosts.contains(&comp) {
                            // Send GPIO event to the IPC adapter.
                            let _ = self.ipc_tx.try_send(IpcResponse::GpioEvent {
                                pin: notif_pin,
                                value: new_val,
                                sim_time: self.now,
                            });
                        } else if let Some(behavior) = self.behaviors.get_mut(&comp) {
                            let mut more = PendingOutputs::default();
                            let mut ctx = RunCtx::new(self.now, &mut more);
                            if let Err(e) = behavior.on_pin_change(notif_pin, new_val, &mut ctx) {
                                tracing::error!(
                                    component = ?comp, error = %e, "on_pin_change failed"
                                );
                            }
                            // Queue cascaded pin writes.
                            cascade.extend(more.pin_writes.drain(..));
                            // Process non-pin outputs inline.
                            if let Some(delay) = more.tick_request {
                                self.tick_heap.push(Reverse((self.now + delay, comp)));
                            }
                            for (ch, pl) in more.mqtt_publishes {
                                if let Some(topic) = self
                                    .publish_routes
                                    .get(&comp)
                                    .and_then(|m| m.get(&ch))
                                    .cloned()
                                {
                                    let _ = self.mqtt_tx.try_send(MqttPublish { topic, payload: pl });
                                }
                            }
                        }
                    }
                }
            } else {
                // Pin is not on any net — update local state only.
                self.pin_states.insert(pin, value);

                // Update SPI CS tracking if this is a CS pin.
                if let Some((bus, cs_name)) = self.cs_pin_to_bus.get(&pin).cloned() {
                    match value {
                        PinValue::Low => { self.spi_active_cs.insert(bus, cs_name); }
                        _ => { self.spi_active_cs.remove(&bus); }
                    }
                }
            }
        }
    }
}
