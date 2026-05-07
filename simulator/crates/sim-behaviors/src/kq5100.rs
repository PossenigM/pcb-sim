//! IFM KQ5100 capacitive level sensor (IO-Link) behavior.
//!
//! Communicates via IO-Link protocol over UART (through the L6360 PHY).
//! Responds to IO-Link M-sequences:
//!   - Startup: TYPE_0 page reads/writes (acknowledge all)
//!   - Preoperate: TYPE_1 ISDU reads/writes (acknowledge all)
//!   - Operate: TYPE_2 process data exchange (returns level value)
//!
//! Process data format (16-bit, big-endian):
//!   bits 15..4: 12-bit level value (0..1645)
//!   bits 3..1:  reserved (0)
//!   bit 0:      switch state OUT1
//!
//! The level value is sourced from an MQTT subscription (float, 0..1645).
//! When the level exceeds 800 (midpoint), switch state OUT1 is set to 1.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

/// IO-Link protocol constants
const PREAMBLE_SEED: u8 = 0x52;
const MC_READ_BIT: u8 = 0x80;
const CKT_TYPE_MASK: u8 = 0xC0;
const CKT_TYPE0: u8 = 0x00;
const CKT_TYPE1: u8 = 0x40;
const CKT_TYPE2: u8 = 0x80;

pub struct Kq5100 {
    level_raw: u16,
    rx_buf: Vec<u8>,
}

impl Kq5100 {
    pub fn new() -> Self {
        Self { level_raw: 0, rx_buf: Vec::new() }
    }

    /// Determine how many bytes are needed for a complete master frame,
    /// given at least 2 bytes (MC + CKT) are available.
    fn frame_length(mc: u8, ckt: u8) -> usize {
        let mseq_type = ckt & CKT_TYPE_MASK;
        match mseq_type {
            CKT_TYPE0 => {
                if (mc & MC_READ_BIT) != 0 { 2 } else { 3 }
            }
            CKT_TYPE1 => 10,
            _ => 3, // TYPE_2: MC + CKT + PDOut
        }
    }

    /// Try to extract one complete IO-Link master frame from the receive buffer.
    fn try_extract_frame(&mut self) -> Option<Vec<u8>> {
        if self.rx_buf.len() < 2 {
            return None;
        }
        let needed = Self::frame_length(self.rx_buf[0], self.rx_buf[1]);
        if self.rx_buf.len() < needed {
            return None;
        }
        Some(self.rx_buf.drain(..needed).collect())
    }

    fn process_data_word(&self) -> u16 {
        let switch_state: u16 = if self.level_raw > 800 { 1 } else { 0 };
        ((self.level_raw & 0x0FFF) << 4) | switch_state
    }

    /// Compute IO-Link checksum6 from XOR-8 accumulator (spec A.1 compression).
    fn checksum6(d: u8) -> u8 {
        let d0 = (d >> 0) & 1;
        let d1 = (d >> 1) & 1;
        let d2 = (d >> 2) & 1;
        let d3 = (d >> 3) & 1;
        let d4 = (d >> 4) & 1;
        let d5 = (d >> 5) & 1;
        let d6 = (d >> 6) & 1;
        let d7 = (d >> 7) & 1;

        let o5 = d7 ^ d5 ^ d3 ^ d1;
        let o4 = d6 ^ d4 ^ d2 ^ d0;
        let o3 = d7 ^ d6;
        let o2 = d5 ^ d4;
        let o1 = d3 ^ d2;
        let o0 = d1 ^ d0;

        (o5 << 5) | (o4 << 4) | (o3 << 3) | (o2 << 2) | (o1 << 1) | o0
    }

    /// Build a TYPE_0 page-read response: [OD, CKS]
    fn build_type0_read_response(&self, mc: u8, _ckt: u8, od: u8) -> Vec<u8> {
        // CKS: type=0 in bits 7..6, checksum6 in bits 5..0
        // PD valid (bit 6 of CKS = 0), no event (bit 7 = 0)
        let mut x = PREAMBLE_SEED;
        x ^= mc;
        x ^= od;
        let ck6 = Self::checksum6(x);
        vec![od, ck6]
    }

    /// Build a TYPE_0 page-write response: [CKS]
    fn build_type0_write_response(&self, mc: u8, _ckt: u8, _od: u8) -> Vec<u8> {
        // Response is just CKS byte (pd_valid=1, no event)
        let x = PREAMBLE_SEED ^ mc;
        let ck6 = Self::checksum6(x);
        vec![ck6]
    }

    /// Build a TYPE_1 read response: [OD×8, CKS]
    fn build_type1_response(&self, mc: u8) -> Vec<u8> {
        // Return 8 zero OD bytes + CKS
        let od = [0u8; 8];
        let mut x = PREAMBLE_SEED;
        x ^= mc;
        for &b in &od {
            x ^= b;
        }
        let ck6 = Self::checksum6(x);
        let mut resp = od.to_vec();
        resp.push(ck6);
        resp
    }

    /// Build a TYPE_2 operate response with process data: [PD_hi, PD_lo, CKS]
    fn build_type2_response(&self, mc: u8) -> Vec<u8> {
        let pd = self.process_data_word();
        let pd_hi = (pd >> 8) as u8;
        let pd_lo = (pd & 0xFF) as u8;
        let mut x = PREAMBLE_SEED;
        x ^= mc;
        x ^= pd_hi;
        x ^= pd_lo;
        let ck6 = Self::checksum6(x);
        vec![pd_hi, pd_lo, ck6]
    }
}

impl Default for Kq5100 {
    fn default() -> Self {
        Self::new()
    }
}

impl IcBehavior for Kq5100 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_mqtt_message(
        &mut self,
        channel: &str,
        payload: MqttValue,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<(), IcError> {
        if channel == "level" {
            if let MqttValue::Float(v) = payload {
                self.level_raw = (v.round() as i32).clamp(0, 1645) as u16;
            }
        }
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::UartFrame { data } => {
                self.rx_buf.extend_from_slice(data);

                let Some(frame) = self.try_extract_frame() else {
                    return Ok(BusResponse::None);
                };

                let mc = frame[0];
                let ckt = frame[1];
                let mseq_type = ckt & CKT_TYPE_MASK;
                let is_read = (mc & MC_READ_BIT) != 0;

                let response = match mseq_type {
                    CKT_TYPE0 => {
                        if is_read {
                            self.build_type0_read_response(mc, ckt, 0x00)
                        } else {
                            let od = frame.get(2).copied().unwrap_or(0);
                            self.build_type0_write_response(mc, ckt, od)
                        }
                    }
                    CKT_TYPE1 => {
                        self.build_type1_response(mc)
                    }
                    CKT_TYPE2 | _ => {
                        self.build_type2_response(mc)
                    }
                };

                Ok(BusResponse::Data(response))
            }
            _ => Err(IcError::Unsupported("kq5100: expected UART IO-Link frame")),
        }
    }
}
