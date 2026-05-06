//! `sim-bin` — the simulator binary.
//!
//! Wires the simulator's pieces together:
//!   1. Parse CLI args.
//!   2. Initialize logging.
//!   3. Load the IC library and validate it.
//!   4. Load the board YAML and cross-validate against the library.
//!   5. Assign numeric IDs to components, pins, and buses.
//!   6. Build behaviors, routers, nets, and routing tables.
//!   7. Instantiate the event loop.
//!   8. Connect MQTT (if configured) and bind IPC sockets.
//!   9. Run until SIGINT.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;

use sim_behaviors;
use sim_config::{
    board::{Board, Bus, Component},
    library::IcLibrary,
    manifest::{IcKind, Interface, InterfaceRole, MqttPayloadType, PinDir},
};
use sim_core::{
    event_loop::{EventLoop, EventLoopConfig, ExternalEvent},
    routers::{
        i2c::I2cRouter,
        net::{Net, NetEndpoint, PinDirection},
        spi::SpiRouter,
        uart::{UartRouter, DEFAULT_UART_BUFFER_BYTES},
    },
    types::{BusId, ComponentId, ConfigValue, PinId},
};
use sim_ipc::IpcAdapter;
use sim_mqtt::{MqttAdapter, PayloadKind, SubscribeRoute};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(name = "sim-bin", version, about = "pcb-sim board simulator")]
struct Cli {
    /// Path to the board YAML file.
    #[arg(short, long)]
    board: PathBuf,

    /// Path to the IC library directory.
    #[arg(short, long, default_value = "./ic-library")]
    library: PathBuf,

    /// Override the MQTT broker URL from the board YAML.
    #[arg(long)]
    broker: Option<String>,

    /// Verbosity. Can be repeated: -v info, -vv debug, -vvv trace.
    #[arg(short, action = clap::ArgAction::Count)]
    verbose: u8,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    tracing::info!(?cli, "starting simulator");

    // ── Load and validate config ──────────────────────────────────────────────

    let library = IcLibrary::load(&cli.library)
        .with_context(|| format!("loading IC library from {}", cli.library.display()))?;

    let board = Board::load_yaml_file(&cli.board)
        .with_context(|| format!("loading board from {}", cli.board.display()))?;

    board.cross_validate(&library)
        .context("board cross-validation failed")?;

    tracing::info!(board = %board.board.name, "board loaded and validated");

    // ── Assign numeric IDs ────────────────────────────────────────────────────

    let mut next_comp:  u32 = 0;
    let mut next_pin:   u32 = 0;
    let mut next_bus:   u32 = 0;

    // component string id → ComponentId
    let comp_ids: HashMap<String, ComponentId> = board.components.iter()
        .map(|c| { let id = ComponentId(next_comp); next_comp += 1; (c.id.clone(), id) })
        .collect();

    // bus string id → BusId
    let bus_ids: HashMap<String, BusId> = board.buses.iter()
        .map(|b| {
            let str_id = bus_str_id(b).to_owned();
            let id = BusId(next_bus);
            next_bus += 1;
            (str_id, id)
        })
        .collect();

    // Per-component: pin name → PinId
    let mut pin_maps: HashMap<ComponentId, HashMap<String, PinId>> = HashMap::new();
    for comp in &board.components {
        let cid = comp_ids[&comp.id];
        let manifest = &library.lookup(&comp.type_ref).unwrap().manifest;
        let mut pmap = HashMap::new();
        for pin in &manifest.pins {
            pmap.insert(pin.name.clone(), PinId(next_pin));
            next_pin += 1;
        }
        pin_maps.insert(cid, pmap);
    }

    // Per-component: interface name → BusId
    let mut bus_maps: HashMap<ComponentId, HashMap<String, BusId>> = HashMap::new();
    for bus in &board.buses {
        let bid = bus_ids[bus_str_id(bus)];
        for (comp_str, iface_name) in bus_members(bus) {
            let cid = comp_ids[comp_str];
            bus_maps.entry(cid).or_default().insert(iface_name.to_owned(), bid);
        }
    }

    // ── Build behaviors and identify firmware hosts ───────────────────────────

    let mut behaviors = HashMap::new();
    let mut firmware_hosts = HashSet::new();
    let mut configs: HashMap<ComponentId, HashMap<String, ConfigValue>> = HashMap::new();

    for comp in &board.components {
        let cid = comp_ids[&comp.id];
        let manifest = &library.lookup(&comp.type_ref).unwrap().manifest;

        // Config: convert serde_yaml::Value → ConfigValue
        let mut cfg_map = HashMap::new();
        for (k, v) in &comp.config {
            if let Some(cv) = yaml_to_config_value(v) {
                cfg_map.insert(k.clone(), cv);
            }
        }
        configs.insert(cid, cfg_map);

        if manifest.kind == IcKind::FirmwareHost {
            firmware_hosts.insert(cid);
            continue;
        }

        if let Some(behavior_key) = &manifest.behavior {
            let key = behavior_key.strip_prefix("builtin:").unwrap_or(behavior_key);
            let behavior = sim_behaviors::registry(key)
                .ok_or_else(|| anyhow!("component '{}': unknown behavior '{}'", comp.id, behavior_key))?;
            behaviors.insert(cid, behavior);
        }
    }

    // ── Build routers ─────────────────────────────────────────────────────────

    let comp_by_id: HashMap<&str, &Component> = board.components.iter()
        .map(|c| (c.id.as_str(), c)).collect();

    let mut i2c_routers = Vec::new();
    let mut spi_routers = Vec::new();
    let mut uart_routers = Vec::new();
    let mut cs_pin_to_bus = HashMap::new();

    for bus in &board.buses {
        let bid = bus_ids[bus_str_id(bus)];

        match bus {
            Bus::I2c { members, .. } => {
                let mut master = None;
                let mut slaves: HashMap<u8, ComponentId> = HashMap::new();

                for member in members {
                    let cid = comp_ids[&member.component];
                    let manifest = &library.lookup(&comp_by_id[member.component.as_str()].type_ref).unwrap().manifest;
                    let iface = manifest.interfaces.iter().find(|i| i.name == member.interface).unwrap();

                    match iface.role {
                        Some(InterfaceRole::Master) => master = Some(cid),
                        _ => {
                            let comp = comp_by_id[member.component.as_str()];
                            if let Some(addr) = resolve_i2c_address(comp, iface) {
                                slaves.insert(addr as u8, cid);
                            } else {
                                tracing::warn!(
                                    component = %member.component,
                                    "I2C slave has no resolvable address; skipping"
                                );
                            }
                        }
                    }
                }

                if let Some(m) = master {
                    i2c_routers.push(I2cRouter::new(bid, m, slaves));
                }
            }

            Bus::Spi { master, slaves, .. } => {
                let master_cid = comp_ids[&master.component];
                let master_pins = &pin_maps[&master_cid];
                let mut slaves_by_cs = HashMap::new();

                for slave in slaves {
                    let slave_cid = comp_ids[&slave.component];
                    slaves_by_cs.insert(slave.cs_pin_on_master.clone(), slave_cid);

                    // Map the master's CS PinId → (BusId, cs_pin_name) for the event loop.
                    if let Some(&cs_pid) = master_pins.get(&slave.cs_pin_on_master) {
                        cs_pin_to_bus.insert(cs_pid, (bid, slave.cs_pin_on_master.clone()));
                    }
                }

                spi_routers.push(SpiRouter::new(bid, master_cid, slaves_by_cs));
            }

            Bus::Uart { peers, .. } => {
                if peers.len() == 2 {
                    let peer_a = comp_ids[&peers[0].component];
                    let peer_b = comp_ids[&peers[1].component];
                    uart_routers.push(UartRouter::new(bid, peer_a, peer_b, DEFAULT_UART_BUFFER_BYTES));
                } else {
                    return Err(anyhow!("UART bus '{}' must have exactly 2 peers", bus_str_id(bus)));
                }
            }
        }
    }

    // ── Build nets ────────────────────────────────────────────────────────────

    let mut nets = Vec::new();
    for (net_idx, net) in board.nets.iter().enumerate() {
        let mut endpoints = Vec::new();
        for ep in &net.endpoints {
            let cid = comp_ids[&ep.component];
            let pid = pin_maps[&cid][&ep.pin];
            let manifest = &library.lookup(&comp_by_id[ep.component.as_str()].type_ref).unwrap().manifest;
            let pin_def = manifest.pins.iter().find(|p| p.name == ep.pin).unwrap();
            let dir = match pin_def.dir {
                PinDir::In    => PinDirection::In,
                PinDir::Out   => PinDirection::Out,
                PinDir::Bidir => PinDirection::Bidir,
            };
            endpoints.push(NetEndpoint { component: cid, pin: pid, pin_dir: dir });
        }
        nets.push(Net::new(net_idx as u32, endpoints));
    }

    // ── Build MQTT routing tables ─────────────────────────────────────────────

    let mqtt_prefix = board.mqtt.as_ref()
        .and_then(|m| m.prefix.as_deref())
        .unwrap_or("");

    let mut publish_routes: HashMap<ComponentId, HashMap<String, String>> = HashMap::new();
    let mut subscribe_routes: HashMap<String, SubscribeRoute> = HashMap::new();

    for comp in &board.components {
        let cid = comp_ids[&comp.id];
        let manifest = &library.lookup(&comp.type_ref).unwrap().manifest;
        let manifest_mqtt = manifest.mqtt.as_ref();

        let Some(comp_mqtt) = &comp.mqtt else { continue };

        // Publish: channel → fully-resolved topic (for event loop → MQTT adapter routing).
        let mut pub_routes = HashMap::new();
        for (channel, relative_topic) in &comp_mqtt.publish {
            pub_routes.insert(channel.clone(), resolve_topic(mqtt_prefix, relative_topic));
        }
        if !pub_routes.is_empty() {
            publish_routes.insert(cid, pub_routes);
        }

        // Subscribe: fully-resolved topic → SubscribeRoute (for MQTT adapter → event loop routing).
        for (channel, relative_topic) in &comp_mqtt.subscribe {
            let topic = resolve_topic(mqtt_prefix, relative_topic);
            let payload_kind = manifest_mqtt
                .and_then(|m| m.subscribe.iter().find(|c| &c.name == channel))
                .map(|c| convert_payload_kind(c.kind))
                .unwrap_or(PayloadKind::String);

            subscribe_routes.insert(topic, SubscribeRoute {
                component: cid,
                channel: channel.clone(),
                payload_kind,
            });
        }
    }

    // ── Build IPC adapters (v1: at most one firmware host) ────────────────────

    let mut ipc_adapters = Vec::new();
    for comp in &board.components {
        let cid = comp_ids[&comp.id];
        if !firmware_hosts.contains(&cid) {
            continue;
        }
        let fw = comp.firmware.as_ref()
            .ok_or_else(|| anyhow!("firmware_host '{}' has no 'firmware:' block", comp.id))?;
        ipc_adapters.push(IpcAdapter::new(
            comp.id.clone(),
            cid,
            PathBuf::from(&fw.path),
            pin_maps.get(&cid).cloned().unwrap_or_default(),
            bus_maps.get(&cid).cloned().unwrap_or_default(),
        ));
    }

    if ipc_adapters.len() > 1 {
        return Err(anyhow!("v1 supports exactly one firmware_host; found {}", ipc_adapters.len()));
    }

    // ── Assemble EventLoopConfig ──────────────────────────────────────────────

    let cfg = EventLoopConfig {
        behaviors,
        pin_maps,
        bus_maps,
        configs,
        i2c_routers,
        spi_routers,
        uart_routers,
        nets,
        cs_pin_to_bus,
        publish_routes,
        firmware_hosts,
    };

    let (mut event_loop, handles) = EventLoop::new(cfg)
        .context("failed to initialize event loop")?;

    let event_tx = handles.event_sender;

    // ── Spawn IPC adapter ─────────────────────────────────────────────────────

    if let Some(adapter) = ipc_adapters.into_iter().next() {
        let sender = event_tx.clone();
        let rx = handles.ipc_response_rx;
        tokio::spawn(async move {
            if let Err(e) = adapter.run(sender, rx).await {
                tracing::error!("IPC adapter exited with error: {e}");
            }
        });
    }

    // ── Spawn MQTT adapter ────────────────────────────────────────────────────

    let broker_url = cli.broker.as_deref()
        .or_else(|| board.mqtt.as_ref()?.broker.as_deref());

    if let Some(broker) = broker_url {
        let mqtt = MqttAdapter::new(broker, "pcb-sim", subscribe_routes)
            .with_context(|| format!("connecting to MQTT broker '{broker}'"))?;
        let sender = event_tx.clone();
        let rx = handles.mqtt_publish_rx;
        tokio::spawn(async move {
            mqtt.run(sender, rx).await;
        });
    }

    // ── SIGINT → Shutdown ─────────────────────────────────────────────────────

    let signal_tx = event_tx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("SIGINT received — shutting down");
            signal_tx.send(ExternalEvent::Shutdown).await;
        }
    });

    // ── Run ───────────────────────────────────────────────────────────────────

    tracing::info!("simulator running");
    event_loop.run().await;
    tracing::info!("simulator stopped");

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bus_str_id(bus: &Bus) -> &str {
    match bus {
        Bus::I2c  { id, .. } => id,
        Bus::Spi  { id, .. } => id,
        Bus::Uart { id, .. } => id,
    }
}

/// Iterate (component_id_str, interface_name) pairs for every bus member.
fn bus_members(bus: &Bus) -> Vec<(&str, &str)> {
    match bus {
        Bus::I2c { members, .. } =>
            members.iter().map(|m| (m.component.as_str(), m.interface.as_str())).collect(),
        Bus::Spi { master, slaves, .. } => {
            let mut v: Vec<(&str, &str)> = vec![(master.component.as_str(), master.interface.as_str())];
            for s in slaves { v.push((s.component.as_str(), s.interface.as_str())); }
            v
        }
        Bus::Uart { peers, .. } =>
            peers.iter().map(|p| (p.component.as_str(), p.interface.as_str())).collect(),
    }
}

/// Resolve a relative MQTT topic against the board prefix.
fn resolve_topic(prefix: &str, relative: &str) -> String {
    if prefix.is_empty() {
        relative.to_owned()
    } else {
        format!("{prefix}/{relative}")
    }
}

/// Convert `sim_config::manifest::MqttPayloadType` → `sim_mqtt::PayloadKind`.
fn convert_payload_kind(kind: MqttPayloadType) -> PayloadKind {
    match kind {
        MqttPayloadType::Bool   => PayloadKind::Bool,
        MqttPayloadType::Int    => PayloadKind::Int,
        MqttPayloadType::Float  => PayloadKind::Float,
        MqttPayloadType::String => PayloadKind::String,
        MqttPayloadType::Bytes  => PayloadKind::Bytes,
    }
}

/// Convert a `serde_yaml::Value` from a component config block into a `ConfigValue`.
fn yaml_to_config_value(v: &serde_yaml::Value) -> Option<ConfigValue> {
    match v {
        serde_yaml::Value::Number(n) => n.as_f64().map(ConfigValue::Number),
        serde_yaml::Value::String(s) => Some(ConfigValue::String(s.clone())),
        serde_yaml::Value::Bool(b)   => Some(ConfigValue::Bool(*b)),
        serde_yaml::Value::Mapping(m) => {
            let mut map = HashMap::new();
            for (k, val) in m {
                if let (Some(ks), Some(vs)) = (k.as_str(), val.as_str()) {
                    map.insert(ks.to_owned(), vs.to_owned());
                }
            }
            if map.is_empty() { None } else { Some(ConfigValue::Map(map)) }
        }
        _ => None,
    }
}

/// Resolve an I2C slave address from board config and/or manifest default.
/// Board config key `{interface_name}.address` takes priority over manifest default.
fn resolve_i2c_address(comp: &Component, iface: &Interface) -> Option<i64> {
    let config_key = format!("{}.address", iface.name);

    if let Some(v) = comp.config.get(&config_key) {
        return yaml_as_i64(v);
    }

    iface.config.get("address")
        .and_then(|cf| cf.default.as_ref())
        .and_then(yaml_as_i64)
}

fn yaml_as_i64(v: &serde_yaml::Value) -> Option<i64> {
    match v {
        serde_yaml::Value::Number(n) => n.as_i64(),
        serde_yaml::Value::String(s) => {
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()
            } else {
                s.parse().ok()
            }
        }
        _ => None,
    }
}

// ── Logging ───────────────────────────────────────────────────────────────────

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| level.to_string());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(false)
        .init();
}
