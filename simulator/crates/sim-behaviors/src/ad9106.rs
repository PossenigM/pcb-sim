//! Analog Devices AD9106 4-channel waveform generator SPI behavior.
//!
//! SPI frame format (4 bytes, 2 × uint16 MSB-first):
//!   Word 0: bit 15 = 0 (write) / 1 (read),  bits 14:0 = register address
//!   Word 1: register data (16-bit)
//!
//! Register map (subset used by firmware, SIZE_REGISTER_MAP = 0x61):
//!   0x00  SPICONFIG   — SPI configuration
//!   0x01  POWERCONFIG — power-down control
//!   0x04  DAC4AGAIN   — DAC4 amplitude gain
//!   0x05  DAC3AGAIN   — DAC3 amplitude gain
//!   0x06  DAC2AGAIN   — DAC2 amplitude gain
//!   0x07  DAC1AGAIN   — DAC1 amplitude gain
//!   0x08  DACxRANGE   — output range
//!   0x0D  CALCONFIG   — calibration configuration
//!   0x1D  RAMUPDATE   — update RAM shadow registers
//!   0x1E  PAT_STATUS  — pattern start/stop; bit 0 = PATTERN_START
//!   0x1F  PAT_TYPE    — pattern type
//!   0x22  DAC4DOF     — DAC4 DC offset
//!   0x23  DAC3DOF     — DAC3 DC offset
//!   0x24  DAC2DOF     — DAC2 DC offset
//!   0x25  DAC1DOF     — DAC1 DC offset
//!
//! On PAT_STATUS write with PATTERN_START=1, publishes dac1..4_gain and
//! dac1..4_offset as normalized floats to MQTT.

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

const REG_MAP_SIZE: usize = 0x61;

const REG_PAT_STATUS: usize = 0x1E;
const REG_DAC1AGAIN: usize  = 0x07;
const REG_DAC2AGAIN: usize  = 0x06;
const REG_DAC3AGAIN: usize  = 0x05;
const REG_DAC4AGAIN: usize  = 0x04;
const REG_DAC1DOF: usize    = 0x25;
const REG_DAC2DOF: usize    = 0x24;
const REG_DAC3DOF: usize    = 0x23;
const REG_DAC4DOF: usize    = 0x22;

const PAT_START_BIT: u16 = 0x0001;

pub struct Ad9106 {
    regs: [u16; REG_MAP_SIZE],
}

impl Ad9106 {
    pub fn new() -> Self {
        Self { regs: [0u16; REG_MAP_SIZE] }
    }

    fn publish_outputs(&self, ctx: &mut RunCtx<'_>) {
        // Gain registers are 12-bit (0–0xFFF), normalized to 0.0–1.0
        let gain_norm = |r: usize| self.regs[r] as f64 / 0x0FFF_u16 as f64;
        // Offset registers are signed 12-bit in upper 12 bits, normalized
        let off_norm = |r: usize| (self.regs[r] as i16 as f64) / i16::MAX as f64;

        ctx.mqtt_publish("dac1_gain",   MqttValue::Float(gain_norm(REG_DAC1AGAIN)));
        ctx.mqtt_publish("dac2_gain",   MqttValue::Float(gain_norm(REG_DAC2AGAIN)));
        ctx.mqtt_publish("dac3_gain",   MqttValue::Float(gain_norm(REG_DAC3AGAIN)));
        ctx.mqtt_publish("dac4_gain",   MqttValue::Float(gain_norm(REG_DAC4AGAIN)));
        ctx.mqtt_publish("dac1_offset", MqttValue::Float(off_norm(REG_DAC1DOF)));
        ctx.mqtt_publish("dac2_offset", MqttValue::Float(off_norm(REG_DAC2DOF)));
        ctx.mqtt_publish("dac3_offset", MqttValue::Float(off_norm(REG_DAC3DOF)));
        ctx.mqtt_publish("dac4_offset", MqttValue::Float(off_norm(REG_DAC4DOF)));
    }
}

impl Default for Ad9106 {
    fn default() -> Self { Self::new() }
}

impl IcBehavior for Ad9106 {
    fn init(&mut self, _ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::SpiTransfer { mosi } => {
                if mosi.len() < 4 {
                    return Ok(BusResponse::Data(vec![0u8; mosi.len()]));
                }

                let word0 = ((mosi[0] as u16) << 8) | mosi[1] as u16;
                let word1 = ((mosi[2] as u16) << 8) | mosi[3] as u16;
                let read_flag = (word0 & 0x8000) != 0;
                let addr = (word0 & 0x7FFF) as usize;

                let mut resp = vec![0u8; mosi.len()];

                if read_flag {
                    // Read: return register value in word 1 of response
                    if addr < REG_MAP_SIZE {
                        let val = self.regs[addr];
                        resp[2] = (val >> 8) as u8;
                        resp[3] = (val & 0xFF) as u8;
                    }
                } else {
                    // Write
                    if addr < REG_MAP_SIZE {
                        self.regs[addr] = word1;
                        if addr == REG_PAT_STATUS && (word1 & PAT_START_BIT) != 0 {
                            self.publish_outputs(ctx);
                        }
                    }
                }

                Ok(BusResponse::Data(resp))
            }
            _ => Err(IcError::Unsupported("ad9106: unsupported transaction")),
        }
    }
}
