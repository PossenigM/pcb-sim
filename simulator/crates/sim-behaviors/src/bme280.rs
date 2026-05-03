//! Bosch BME280 behavior.
//!
//! Subscribes to `temperature`, `humidity`, `pressure` MQTT channels.
//! Caches the latest values. Responds to I2C transactions by encoding
//! the cached values in the BME280 register format.
//!
//! Manifest reference: `ic-library/bosch_bme280/manifest.yaml`.
//!
//! Datasheet: BME280 §5 (register map) and §4 (compensation formulas).

use sim_core::{
    BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx,
};

/// Currently-cached sensor values. Initialized from the board YAML's
/// `initial_values` block at simulator startup.
pub struct Bme280 {
    pub temperature_c: f64,
    pub humidity_pct: f64,
    pub pressure_hpa: f64,
    /// The last register address written by the master. Used for the
    /// "set pointer, then read" pattern. None means no pointer yet.
    pub register_pointer: Option<u8>,
}

impl Bme280 {
    pub fn new() -> Self {
        // TODO: real init values come via InitCtx from the board YAML.
        //       This default is a placeholder.
        Self {
            temperature_c: 20.0,
            humidity_pct: 50.0,
            pressure_hpa: 1013.0,
            register_pointer: None,
        }
    }
}

impl Default for Bme280 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Bme280 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        // TODO: read initial_values from ctx and overwrite our defaults.
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        match (channel, payload) {
            ("temperature", MqttValue::Float(v)) => self.temperature_c = v,
            ("humidity",    MqttValue::Float(v)) => self.humidity_pct = v,
            ("pressure",    MqttValue::Float(v)) => self.pressure_hpa = v,
            (other, _) => {
                tracing::warn!(channel = other, "BME280 received unknown MQTT channel");
            }
        }
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        // TODO: implement BME280 register map.
        //   - I2cWrite: first byte sets register pointer; remaining bytes
        //     write registers (BME280 has writable config registers).
        //   - I2cRead: read from current pointer, advancing.
        //   - I2cWriteRead: combined; write phase sets pointer, read phase
        //     returns N bytes from there.
        //   - Register encoding: temperature/pressure/humidity readings
        //     live in registers 0xF7..0xFE. They are *raw ADC values* on a
        //     real chip; firmware applies compensation. For our purposes,
        //     we can either:
        //       (a) implement the BME280 compensation in reverse (encode
        //           SI values as raw ADC bytes), or
        //       (b) cheat and store SI values in a "fake" format that the
        //           firmware HAL knows about (only useful if you control
        //           the firmware).
        //     Recommendation: do (a). It is more work but lets unmodified
        //     drivers run against the simulator.
        let _ = txn;
        Err(IcError::Unsupported("bme280 i2c handling not yet implemented"))
    }
}
