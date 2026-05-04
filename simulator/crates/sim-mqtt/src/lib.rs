//! `sim-mqtt` — MQTT client adapter.
//!
//! Connects to the broker configured in the board YAML. Maintains a
//! topic ↔ (component_id, channel) mapping table built at simulator
//! startup. Forwards incoming messages to the event loop as
//! `IncomingMqtt` events; handles outgoing publishes from `RunCtx`.

use sim_core::{ComponentId, MqttValue};
use std::collections::HashMap;

pub struct MqttAdapter {
    /// Topic → (target component, manifest channel name) for subscriptions.
    pub subscribe_routes: HashMap<String, SubscribeRoute>,
    /// (source component, manifest channel name) → outgoing topic for
    /// publications.
    pub publish_routes: HashMap<(ComponentId, String), String>,
    // TODO: rumqttc client handle, event loop handle, etc.
}

pub struct SubscribeRoute {
    pub component: ComponentId,
    pub channel: String,
}

pub struct IncomingMqtt {
    pub component: ComponentId,
    pub channel: String,
    pub payload: MqttValue,
}

impl MqttAdapter {
    // TODO: pub async fn new(board: &Board, library: &IcLibrary) -> Result<Self, _>
    //   Build the routing tables from a loaded Board and library, then
    //   connect to the broker and subscribe to every topic in subscribe_routes.

    // TODO: pub async fn publish(&self, component: ComponentId, channel: &str, value: MqttValue)
    //   Publish a value on the topic mapped from (component, channel).
    //   No-op with a warning if no mapping exists.

    // TODO: pub async fn run(self, sink: EventSender) -> Result<(), _>
    //   Run the receive loop, decoding payloads and pushing ExternalEvent::MqttMessage
    //   events onto the supplied event loop sender.
}

// TODO: payload decoding helpers. MQTT payloads are bytes; we need to
//       map them to `MqttValue` based on the manifest channel type:
//         bool   -> "true"/"false" or "1"/"0"
//         int    -> ASCII integer
//         float  -> ASCII float
//         string -> as-is UTF-8
//         bytes  -> raw bytes
//       Be lenient on the wire (the physics engine might send any of
//       these encodings) but pick one canonical encoding for our own
//       publishes.
