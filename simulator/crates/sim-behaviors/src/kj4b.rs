//! Kyocera KJ4B printhead behavior.
//!
//! Packet protocol (UART):
//!   TX from master: [0xFE 0xFE 0xFE] [N] [CMD] [PAYLOAD...] [CHECKSUM]
//!   RX response:    [0xFE 0xFE 0xFE] [N] [CMD] [RES1|PAYLOAD...] [CHECKSUM]
//!
//! Where:
//!   N = payload_len + 2 (includes command byte + checksum byte)
//!   CHECKSUM = -(N + CMD + sum(payload)) & 0xFF
//!   RES1 bit 7 = 1 means valid/successful response
//!
//! The behavior simulates a healthy printhead:
//!   - All commands return success (RES1 = 0x80)
//!   - ReadHeaterTemp (0x12) returns the MQTT-subscribed temperature
//!   - WriteTempLimitsForHeaterSetting (0x87) publishes the target to MQTT
//!   - ReadSerialNumber (0x01) returns a fixed serial "SIM00000"
//!   - All other commands return RES1=0x80 with zero-filled payload

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

const PREAMBLE: u8 = 0xFE;
const PREAMBLE_COUNT: usize = 3;
const OVERHEAD: usize = PREAMBLE_COUNT + 3; // preamble + N + cmd + checksum

// NTC conversion constants (matches firmware TempRawToCenti / TempCentiToRaw)
const R25: f64 = 10000.0;
const B_COEFF: f64 = 3435.0;
const T0_INV: f64 = 1.0 / 298.15;

pub struct Kj4b {
    /// Temperature in centi-celsius (e.g. 2500 = 25.00°C)
    temperature_centi: i16,
    /// Heater target raw value (as set by firmware)
    heater_target_raw: u16,
    /// Receive buffer: accumulates UART bytes until a complete packet arrives.
    rx_buf: Vec<u8>,
}

impl Kj4b {
    pub fn new() -> Self {
        Self {
            temperature_centi: 2500, // 25.00°C default
            heater_target_raw: 0,
            rx_buf: Vec::new(),
        }
    }

    /// Try to extract one complete Kyocera packet from the receive buffer.
    /// Returns the packet bytes (including preamble) if complete, or None.
    fn try_extract_packet(&mut self) -> Option<Vec<u8>> {
        // Find preamble (3× 0xFE). Discard any leading junk.
        loop {
            if self.rx_buf.len() < 4 {
                return None;
            }
            if self.rx_buf[0] == PREAMBLE
                && self.rx_buf[1] == PREAMBLE
                && self.rx_buf[2] == PREAMBLE
            {
                break;
            }
            self.rx_buf.remove(0);
        }

        let n = self.rx_buf[3] as usize;
        let total_len = 4 + n; // preamble(3) + N(1) + N bytes (CMD + payload + checksum)
        if self.rx_buf.len() < total_len {
            return None;
        }

        let packet: Vec<u8> = self.rx_buf.drain(..total_len).collect();
        Some(packet)
    }

    /// Convert temperature in centi-celsius to raw NTC ADC value.
    /// Formula: raw = 1023 * R25 / (R25 + R)
    /// where R = R25 * exp(B * (1/T - 1/T0))
    fn temp_centi_to_raw(centi: i16) -> u16 {
        let t_kelvin = (centi as f64) / 100.0 + 273.15;
        let r = R25 * (B_COEFF * (1.0 / t_kelvin - T0_INV)).exp();
        let raw = 1023.0 * R25 / (R25 + r);
        (raw.round() as i32).clamp(0, 1023) as u16
    }

    /// Convert raw NTC ADC value to centi-celsius.
    #[allow(dead_code)]
    fn raw_to_temp_centi(raw: u16) -> i16 {
        if raw == 0 {
            return -4000; // very cold / disconnected
        }
        let r_ratio = (1023.0 - raw as f64) / raw as f64;
        let t_inv = T0_INV + (1.0 / B_COEFF) * r_ratio.ln();
        let t_celsius = 1.0 / t_inv - 273.15;
        (t_celsius * 100.0).round() as i16
    }

    /// Compute Kyocera protocol checksum.
    fn checksum(n: u8, cmd: u8, payload: &[u8]) -> u8 {
        let mut sum: u16 = n as u16 + cmd as u16;
        for &b in payload {
            sum += b as u16;
        }
        -(sum as i16) as u8
    }

    /// Build a response packet for a given command and response payload.
    fn build_response(cmd: u8, payload: &[u8]) -> Vec<u8> {
        let n = (payload.len() as u8) + 2; // payload + cmd + checksum
        let cksum = Self::checksum(n, cmd, payload);
        let mut pkt = Vec::with_capacity(OVERHEAD + payload.len());
        for _ in 0..PREAMBLE_COUNT {
            pkt.push(PREAMBLE);
        }
        pkt.push(n);
        pkt.push(cmd);
        pkt.extend_from_slice(payload);
        pkt.push(cksum);
        pkt
    }

    /// Handle a parsed command and return the response payload.
    fn handle_command(&mut self, cmd: u8, _tx_payload: &[u8], ctx: &mut RunCtx<'_>) -> Vec<u8> {
        match cmd {
            // ReadSerialNumber (0x01): return "SIM00000"
            0x01 => {
                let mut resp = vec![0x80u8]; // RES1 valid
                resp.extend_from_slice(b"SIM0000");
                resp
            }
            // ReadAccumulatedDriveTimes (0x02): return 7 zero bytes
            0x02 => {
                let mut resp = vec![0x80u8]; // RES1 valid
                resp.extend_from_slice(&[0u8; 6]);
                resp
            }
            // ReadHeaterTemp (0x12): return temperature as raw NTC value
            0x12 => {
                let raw = Self::temp_centi_to_raw(self.temperature_centi);
                // Response: RES1 + 7 bytes (raw value in bytes [1..2], rest zero)
                let mut resp = vec![0x80u8]; // RES1 valid
                resp.push((raw >> 8) as u8);
                resp.push((raw & 0xFF) as u8);
                resp.extend_from_slice(&[0u8; 5]);
                resp
            }
            // WriteTempLimitsForHeaterSetting (0x87): set heater target
            0x87 => {
                if _tx_payload.len() >= 2 {
                    self.heater_target_raw =
                        ((_tx_payload[0] as u16) << 8) | (_tx_payload[1] as u16);
                    // Convert raw back to centi and publish
                    let target_centi = Self::raw_to_temp_centi(self.heater_target_raw);
                    ctx.mqtt_publish(
                        "heater_target",
                        MqttValue::Float(target_centi as f64),
                    );
                }
                vec![0x80u8] // RES1 valid (no payload for write commands)
            }
            // All other commands: return success with appropriately sized payload
            _ => {
                vec![0x80u8] // RES1 valid, minimal response
            }
        }
    }
}

impl Default for Kj4b {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Kj4b {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if channel == "temperature" {
            if let MqttValue::Float(v) = payload {
                self.temperature_centi = v.round() as i16;
            }
        }
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::UartFrame { data } => {
                self.rx_buf.extend_from_slice(data);

                let Some(packet) = self.try_extract_packet() else {
                    return Ok(BusResponse::None);
                };

                let n = packet[PREAMBLE_COUNT] as usize;
                let cmd = packet[PREAMBLE_COUNT + 1];
                let payload_len = if n >= 2 { n - 2 } else { 0 };
                let payload_start = PREAMBLE_COUNT + 2;
                let payload_end = payload_start + payload_len;
                let tx_payload = &packet[payload_start..payload_end];

                let resp_payload = self.handle_command(cmd, tx_payload, ctx);
                let response = Self::build_response(cmd, &resp_payload);

                Ok(BusResponse::Data(response))
            }
            _ => Err(IcError::Unsupported("kj4b: expected UART frame")),
        }
    }
}
