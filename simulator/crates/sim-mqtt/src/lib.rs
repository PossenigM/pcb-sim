//! `sim-mqtt` — MQTT client adapter.
//!
//! Connects to the broker configured in the board YAML. Maintains a
//! topic ↔ (component_id, channel) mapping table built at simulator
//! startup. Forwards incoming messages to the event loop as
//! `ExternalEvent::MqttMessage` events; handles outgoing publishes from
//! `MqttPublish` messages produced by behaviors via `RunCtx`.

use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use sim_core::{ComponentId, EventSender, ExternalEvent, MqttPublish, MqttValue};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{info, warn};

// ── Payload kind ──────────────────────────────────────────────────────────────

/// Expected payload type for a subscribed MQTT topic.
///
/// Mirrors `sim_config::manifest::MqttPayloadType` to avoid a dependency
/// on `sim-config` from `sim-mqtt`. `sim-bin` converts between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    Bool,
    Int,
    Float,
    String,
    Bytes,
}

// ── Public types ──────────────────────────────────────────────────────────────

/// Routing entry for one subscribed MQTT topic.
pub struct SubscribeRoute {
    pub component: ComponentId,
    pub channel: String,
    pub payload_kind: PayloadKind,
}

// ── MqttAdapter ───────────────────────────────────────────────────────────────

pub struct MqttAdapter {
    subscribe_routes: HashMap<String, SubscribeRoute>,
    client: AsyncClient,
    eventloop: EventLoop,
}

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("invalid broker URL '{0}': expected 'host[:port]', 'mqtt://host[:port]', or 'tcp://host[:port]'")]
    BadUrl(String),
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),
}

// ── Constructor ───────────────────────────────────────────────────────────────

impl MqttAdapter {
    /// Create the adapter.
    ///
    /// Parses `broker` (accepts `"host"`, `"host:port"`, `"mqtt://host:port"`,
    /// `"tcp://host:port"`; default port 1883) and sets up the internal
    /// rumqttc client/eventloop pair. Does **not** connect — connection happens
    /// when `run()` polls the event loop for the first time.
    pub fn new(
        broker: &str,
        client_id: &str,
        subscribe_routes: HashMap<String, SubscribeRoute>,
    ) -> Result<Self, MqttError> {
        let (host, port) = parse_broker_url(broker)?;
        let mut opts = MqttOptions::new(client_id, host, port);
        opts.set_keep_alive(std::time::Duration::from_secs(30));
        opts.set_clean_session(true);

        let (client, eventloop) = AsyncClient::new(opts, 64);
        Ok(Self { subscribe_routes, client, eventloop })
    }

    /// Run the adapter until the event loop shuts down.
    ///
    /// - Subscribes to all topics on (re)connect via `ConnAck`.
    /// - Dispatches incoming `Publish` packets to the simulator event loop.
    /// - Forwards `MqttPublish` messages from behaviors to the broker.
    ///
    /// Call this in a `tokio::spawn`ed task alongside the simulator event loop.
    pub async fn run(
        self,
        event_tx: EventSender,
        mut publish_rx: mpsc::Receiver<MqttPublish>,
    ) {
        let Self { subscribe_routes, client, mut eventloop } = self;

        // Pre-collect subscribe topics so we can iterate without borrowing
        // `subscribe_routes` inside the eventloop poll loop.
        let sub_topics: Vec<String> = subscribe_routes.keys().cloned().collect();

        // Spawn a task to forward behavior-generated publishes to the broker.
        // `AsyncClient::publish` just enqueues into the event loop's channel;
        // the actual send happens when `eventloop.poll()` is called below.
        let client_pub = client.clone();
        let publish_task = tokio::spawn(async move {
            while let Some(msg) = publish_rx.recv().await {
                let payload = encode_payload(&msg.payload);
                if let Err(e) = client_pub
                    .publish(&msg.topic, QoS::AtMostOnce, false, payload)
                    .await
                {
                    warn!(topic = %msg.topic, "MQTT publish failed: {e}");
                }
            }
        });

        // Drive the rumqttc event loop. This processes both inbound packets
        // and the outbound queue filled by the publish task above.
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    // (Re)subscribe after every connect so clean-session
                    // reconnects don't lose subscriptions.
                    info!("MQTT connected; subscribing to {} topic(s)", sub_topics.len());
                    for topic in &sub_topics {
                        if let Err(e) = client.subscribe(topic, QoS::AtMostOnce).await {
                            warn!(topic, "MQTT subscribe failed: {e}");
                        }
                    }
                }

                Ok(Event::Incoming(Packet::Publish(p))) => {
                    let topic = p.topic.as_str();
                    if let Some(route) = subscribe_routes.get(topic) {
                        match decode_payload(&p.payload, route.payload_kind) {
                            Ok(value) => {
                                let sent = event_tx
                                    .send(ExternalEvent::MqttMessage {
                                        component: route.component,
                                        channel: route.channel.clone(),
                                        payload: value,
                                    })
                                    .await;
                                if !sent {
                                    // The simulator event loop has shut down.
                                    break;
                                }
                            }
                            Err(e) => warn!(topic, "MQTT: cannot decode payload: {e}"),
                        }
                    } else {
                        warn!(topic, "MQTT: received publish for unknown topic");
                    }
                }

                Ok(_) => {}

                Err(e) => {
                    // rumqttc automatically retries the connection on the next
                    // poll(); log and continue.
                    warn!("MQTT connection error: {e}");
                }
            }
        }

        publish_task.abort();
    }
}

// ── URL parsing ───────────────────────────────────────────────────────────────

fn parse_broker_url(url: &str) -> Result<(String, u16), MqttError> {
    let host_port = url
        .strip_prefix("mqtt://")
        .or_else(|| url.strip_prefix("tcp://"))
        .unwrap_or(url);

    if let Some((host, port_str)) = host_port.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| MqttError::BadUrl(url.to_string()))?;
        Ok((host.to_string(), port))
    } else {
        Ok((host_port.to_string(), 1883))
    }
}

// ── Payload codec ─────────────────────────────────────────────────────────────

/// Encode an `MqttValue` into raw MQTT bytes for publishing.
///
/// Canonical wire format used by the simulator for its own publishes:
/// - `Bool`   → `"true"` / `"false"` (UTF-8)
/// - `Int`    → ASCII decimal
/// - `Float`  → ASCII decimal (Rust's `f64::to_string`)
/// - `String` → UTF-8 bytes
/// - `Bytes`  → raw bytes
pub fn encode_payload(value: &MqttValue) -> Vec<u8> {
    match value {
        MqttValue::Bool(b)   => if *b { b"true".to_vec() } else { b"false".to_vec() },
        MqttValue::Int(n)    => n.to_string().into_bytes(),
        MqttValue::Float(f)  => f.to_string().into_bytes(),
        MqttValue::String(s) => s.as_bytes().to_vec(),
        MqttValue::Bytes(b)  => b.clone(),
    }
}

/// Decode raw MQTT bytes into an `MqttValue` according to the declared
/// channel type. Lenient: accepts `"1"`/`"0"` for bools, leading/trailing
/// whitespace is stripped for text types.
pub fn decode_payload(raw: &[u8], kind: PayloadKind) -> Result<MqttValue, String> {
    match kind {
        PayloadKind::Bool => {
            let s = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
            match s.trim() {
                "true" | "1"  => Ok(MqttValue::Bool(true)),
                "false" | "0" => Ok(MqttValue::Bool(false)),
                other => Err(format!("expected bool ('true'/'false'/'1'/'0'), got '{other}'")),
            }
        }
        PayloadKind::Int => {
            let s = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
            s.trim()
                .parse::<i64>()
                .map(MqttValue::Int)
                .map_err(|e| e.to_string())
        }
        PayloadKind::Float => {
            let s = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
            s.trim()
                .parse::<f64>()
                .map(MqttValue::Float)
                .map_err(|e| e.to_string())
        }
        PayloadKind::String => std::str::from_utf8(raw)
            .map(|s| MqttValue::String(s.to_string()))
            .map_err(|e| e.to_string()),
        PayloadKind::Bytes => Ok(MqttValue::Bytes(raw.to_vec())),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_broker_url_plain_host() {
        let (host, port) = parse_broker_url("localhost").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 1883);
    }

    #[test]
    fn parse_broker_url_host_port() {
        let (host, port) = parse_broker_url("broker.example.com:1884").unwrap();
        assert_eq!(host, "broker.example.com");
        assert_eq!(port, 1884);
    }

    #[test]
    fn parse_broker_url_mqtt_scheme() {
        let (host, port) = parse_broker_url("mqtt://localhost:1883").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 1883);
    }

    #[test]
    fn parse_broker_url_tcp_scheme() {
        let (host, port) = parse_broker_url("tcp://10.0.0.1:8883").unwrap();
        assert_eq!(host, "10.0.0.1");
        assert_eq!(port, 8883);
    }

    #[test]
    fn parse_broker_url_bad_port() {
        assert!(parse_broker_url("localhost:notaport").is_err());
    }

    #[test]
    fn encode_decode_bool() {
        let enc = encode_payload(&MqttValue::Bool(true));
        assert_eq!(enc, b"true");
        assert_eq!(decode_payload(&enc, PayloadKind::Bool).unwrap(), MqttValue::Bool(true));
        assert_eq!(decode_payload(b"1", PayloadKind::Bool).unwrap(), MqttValue::Bool(true));
        assert_eq!(decode_payload(b"false", PayloadKind::Bool).unwrap(), MqttValue::Bool(false));
        assert_eq!(decode_payload(b"0", PayloadKind::Bool).unwrap(), MqttValue::Bool(false));
    }

    #[test]
    fn encode_decode_int() {
        let enc = encode_payload(&MqttValue::Int(-42));
        assert_eq!(enc, b"-42");
        assert_eq!(decode_payload(&enc, PayloadKind::Int).unwrap(), MqttValue::Int(-42));
    }

    #[test]
    fn encode_decode_float() {
        let enc = encode_payload(&MqttValue::Float(3.14));
        assert_eq!(decode_payload(&enc, PayloadKind::Float).unwrap(), MqttValue::Float(3.14));
    }

    #[test]
    fn encode_decode_string() {
        let enc = encode_payload(&MqttValue::String("hello".into()));
        assert_eq!(decode_payload(&enc, PayloadKind::String).unwrap(), MqttValue::String("hello".into()));
    }

    #[test]
    fn encode_decode_bytes() {
        let enc = encode_payload(&MqttValue::Bytes(vec![0xde, 0xad]));
        assert_eq!(decode_payload(&enc, PayloadKind::Bytes).unwrap(), MqttValue::Bytes(vec![0xde, 0xad]));
    }

    #[test]
    fn decode_int_trims_whitespace() {
        assert_eq!(decode_payload(b" 7 \n", PayloadKind::Int).unwrap(), MqttValue::Int(7));
    }
}
