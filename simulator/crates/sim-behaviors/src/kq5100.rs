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
const CKS_STATUS_OK: u8 = 0x00;
const KQ5100_PRODUCT_NAME: &[u8] = b"KQ5100";
const ISDU_CONTROL_ADDR: u8 = 16;
const ISDU_INDEX_LSB_ADDR: u8 = 1;
const ISDU_SERVICE_QUAL_ADDR: u8 = 2;
const PRODUCT_NAME_INDEX: u16 = 18;

pub struct Kq5100 {
    level_raw: u16,
    rx_buf: Vec<u8>,
    isdu_index_lsb: u8,
    isdu_index: u16,
}

impl Kq5100 {
    pub fn new() -> Self {
        Self {
            level_raw: 0,
            rx_buf: Vec::new(),
            isdu_index_lsb: 0,
            isdu_index: 0,
        }
    }

    /// Determine how many bytes are needed for a complete master frame,
    /// given at least 2 bytes (MC + CKT) are available.
    fn frame_length(mc: u8, ckt: u8) -> usize {
        let mseq_type = ckt & CKT_TYPE_MASK;
        let is_read = (mc & MC_READ_BIT) != 0;
        match mseq_type {
            CKT_TYPE0 => {
                if is_read { 2 } else { 3 }
            }
            CKT_TYPE1 => {
                if is_read { 2 } else { 10 }
            }
            _ => {
                if is_read { 2 } else { 3 }
            }
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

    fn response_cks(payload: &[u8], status_bits: u8) -> u8 {
        let status_bits = status_bits & CKT_TYPE_MASK;
        let mut x = PREAMBLE_SEED ^ status_bits;
        for &b in payload {
            x ^= b;
        }
        status_bits | Self::checksum6(x)
    }

    fn page_data(address: u8) -> u8 {
        match address {
            // Values read by firmware during startup validation.
            7 => 0x03,
            8 => 0x10,
            9 => 0x00,
            10 => 0x03,
            11 => 0x71,
            _ => 0x00,
        }
    }

    fn record_type2_write(&mut self, mc: u8, pdout: u8) {
        match mc & 0x1F {
            ISDU_INDEX_LSB_ADDR => {
                self.isdu_index_lsb = pdout;
            }
            ISDU_SERVICE_QUAL_ADDR => {
                let index_msb = ((pdout & 0x7E) >> 1) as u16;
                self.isdu_index = (index_msb << 8) | self.isdu_index_lsb as u16;
            }
            _ => {}
        }
    }

    fn isdu_read_data(&self, address: u8) -> u8 {
        if self.isdu_index == PRODUCT_NAME_INDEX && address >= ISDU_INDEX_LSB_ADDR {
            let offset = (address - ISDU_INDEX_LSB_ADDR) as usize;
            return KQ5100_PRODUCT_NAME.get(offset).copied().unwrap_or(0);
        }
        0
    }

    /// Build a TYPE_0 page-read response: [OD, CKS]
    fn build_type0_read_response(&self, od: u8) -> Vec<u8> {
        vec![od, Self::response_cks(&[od], CKS_STATUS_OK)]
    }

    /// Build a TYPE_0 page-write response: [CKS]
    fn build_type0_write_response(&self) -> Vec<u8> {
        vec![Self::response_cks(&[], CKS_STATUS_OK)]
    }

    /// Build a TYPE_1 read response: [OD×8, CKS]
    fn build_type1_read_response(&self) -> Vec<u8> {
        let od = [0u8; 8];
        let mut resp = od.to_vec();
        resp.push(Self::response_cks(&od, CKS_STATUS_OK));
        resp
    }

    /// Build a TYPE_1 write acknowledgement: [CKS]
    fn build_type1_write_response(&self) -> Vec<u8> {
        vec![Self::response_cks(&[], CKS_STATUS_OK)]
    }

    /// Build a TYPE_2 read response: [PDI, OD0, OD1, CKS]
    fn build_type2_read_response(&self, address: u8) -> Vec<u8> {
        let pdi = if address == ISDU_CONTROL_ADDR {
            0
        } else {
            self.isdu_read_data(address)
        };
        let payload = [pdi, 0u8, 0u8];
        vec![
            payload[0],
            payload[1],
            payload[2],
            Self::response_cks(&payload, CKS_STATUS_OK),
        ]
    }

    /// Build a TYPE_2 write/cyclic response with process data: [PD_hi, PD_lo, CKS]
    fn build_type2_write_response(&self) -> Vec<u8> {
        let pd = self.process_data_word();
        let pd_hi = (pd >> 8) as u8;
        let pd_lo = (pd & 0xFF) as u8;
        let payload = [pd_hi, pd_lo];
        vec![pd_hi, pd_lo, Self::response_cks(&payload, CKS_STATUS_OK)]
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
                            self.build_type0_read_response(Self::page_data(mc & 0x1F))
                        } else {
                            self.build_type0_write_response()
                        }
                    }
                    CKT_TYPE1 => {
                        if is_read {
                            self.build_type1_read_response()
                        } else {
                            self.build_type1_write_response()
                        }
                    }
                    CKT_TYPE2 | _ => {
                        if is_read {
                            self.build_type2_read_response(mc & 0x1F)
                        } else {
                            if let Some(pdout) = frame.get(2).copied() {
                                self.record_type2_write(mc, pdout);
                            }
                            self.build_type2_write_response()
                        }
                    }
                };

                Ok(BusResponse::Data(response))
            }
            _ => Err(IcError::Unsupported("kq5100: expected UART IO-Link frame")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn firmware_accepts(payload: &[u8], cks: u8) -> bool {
        let mut x = PREAMBLE_SEED ^ (cks & CKT_TYPE_MASK);
        for &b in payload {
            x ^= b;
        }
        Kq5100::checksum6(x) == (cks & 0x3F)
    }

    #[test]
    fn type0_read_checksum_matches_firmware_verifier() {
        let sensor = Kq5100::new();
        let resp = sensor.build_type0_read_response(0x36);
        assert_eq!(resp.len(), 2);
        assert!(firmware_accepts(&resp[..1], resp[1]));
    }

    #[test]
    fn type0_write_ack_checksum_matches_firmware_verifier() {
        let sensor = Kq5100::new();
        let resp = sensor.build_type0_write_response();
        assert_eq!(resp.len(), 1);
        assert!(firmware_accepts(&[], resp[0]));
    }

    #[test]
    fn frame_length_distinguishes_reads_and_writes() {
        assert_eq!(Kq5100::frame_length(0x80, CKT_TYPE0), 2);
        assert_eq!(Kq5100::frame_length(0x00, CKT_TYPE0), 3);
        assert_eq!(Kq5100::frame_length(0x80, CKT_TYPE1), 2);
        assert_eq!(Kq5100::frame_length(0x00, CKT_TYPE1), 10);
        assert_eq!(Kq5100::frame_length(0x80, CKT_TYPE2), 2);
        assert_eq!(Kq5100::frame_length(0x00, CKT_TYPE2), 3);
    }

    #[test]
    fn response_lengths_match_firmware_reads() {
        let sensor = Kq5100::new();

        let type1_read = sensor.build_type1_read_response();
        assert_eq!(type1_read.len(), 9);
        assert!(firmware_accepts(&type1_read[..8], type1_read[8]));

        let type1_write = sensor.build_type1_write_response();
        assert_eq!(type1_write.len(), 1);
        assert!(firmware_accepts(&[], type1_write[0]));

        let type2_read = sensor.build_type2_read_response(ISDU_CONTROL_ADDR);
        assert_eq!(type2_read.len(), 4);
        assert!(firmware_accepts(&type2_read[..3], type2_read[3]));

        let type2_write = sensor.build_type2_write_response();
        assert_eq!(type2_write.len(), 3);
        assert!(firmware_accepts(&type2_write[..2], type2_write[2]));
    }

    #[test]
    fn page_data_exposes_expected_ids() {
        assert_eq!(Kq5100::page_data(7), 0x03);
        assert_eq!(Kq5100::page_data(8), 0x10);
        assert_eq!(Kq5100::page_data(9), 0x00);
        assert_eq!(Kq5100::page_data(10), 0x03);
        assert_eq!(Kq5100::page_data(11), 0x71);
    }

    #[test]
    fn type2_isdu_read_exposes_product_name() {
        let mut sensor = Kq5100::new();

        sensor.record_type2_write(ISDU_INDEX_LSB_ADDR, PRODUCT_NAME_INDEX as u8);
        sensor.record_type2_write(ISDU_SERVICE_QUAL_ADDR, 0x81);

        for (offset, expected) in KQ5100_PRODUCT_NAME.iter().enumerate() {
            let address = ISDU_INDEX_LSB_ADDR + offset as u8;
            let resp = sensor.build_type2_read_response(address);
            assert_eq!(resp[0], *expected);
            assert!(firmware_accepts(&resp[..3], resp[3]));
        }

        let terminator = sensor.build_type2_read_response(
            ISDU_INDEX_LSB_ADDR + KQ5100_PRODUCT_NAME.len() as u8,
        );
        assert_eq!(terminator[0], 0);
        assert!(firmware_accepts(&terminator[..3], terminator[3]));
    }
}
