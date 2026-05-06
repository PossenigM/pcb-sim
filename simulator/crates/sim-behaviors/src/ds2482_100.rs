//! Maxim DS2482-100 I2C-to-1-Wire bridge with virtual DS18X20.
//!
//! I2C command protocol (address configured in board YAML):
//!   0xF0          Device Reset
//!   0xD2 <cfg>    Write Configuration
//!   0xB4          1-Wire Reset (marks presence)
//!   0xA5 <byte>   1-Wire Write Byte (accumulates into 1-Wire command buffer)
//!   0x96          1-Wire Read Byte (primes the read-data register)
//!   0xE1 <ptr>    Set Read Pointer (0xF0=status, 0xE1=read_data, 0xC3=config)
//!
//! I2C reads return the register currently pointed to by the read pointer.
//!
//! Status register bit layout:
//!   bit 0: 1WB  (1-Wire busy — always 0 in simulation)
//!   bit 1: PPD  (presence pulse detected — 1 after 1-Wire reset)
//!   bit 4: RST  (device reset — 1 after device reset)
//!
//! DS18X20 virtual 1-Wire state machine:
//!   After [0xCC, 0x44]  (SkipROM + ConvertT):  temperature conversion latched
//!   After [0xCC, 0xBE]  (SkipROM + ReadScratchpad): subsequent Read Byte
//!     operations return the 9-byte scratchpad
//!
//! Scratchpad encoding (DS18B20, 12-bit mode):
//!   bytes 0–1: temperature word (signed 16-bit, LSB = 1/16 °C)
//!   bytes 2–8: fixed values (TH=0, TL=0, cfg=0x7F, reserved, CRC=0)

use sim_core::{BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, MqttValue, RunCtx};

// DS2482 command codes
const CMD_RESET: u8       = 0xF0;
const CMD_WRITE_CFG: u8   = 0xD2;
const CMD_OW_RESET: u8    = 0xB4;
const CMD_OW_WRITE: u8    = 0xA5;
const CMD_OW_READ: u8     = 0x96;
const CMD_SET_PTR: u8     = 0xE1;

// Read-pointer codes
const PTR_STATUS: u8    = 0xF0;
const PTR_READ_DATA: u8 = 0xE1;
const PTR_CONFIG: u8    = 0xC3;

// DS18X20 1-Wire commands
const OW_SKIP_ROM: u8         = 0xCC;
const OW_CONVERT_T: u8        = 0x44;
const OW_READ_SCRATCHPAD: u8  = 0xBE;

// Status bits
const STATUS_PPD: u8 = 0x02;
const STATUS_RST: u8 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq)]
enum OWState {
    Idle,
    RomCmd,
    ConvertT,
    ReadScratchpad { idx: usize },
    RomCmd2,
}

pub struct Ds2482_100 {
    temperature_c: f64,
    // DS2482 internal registers
    status_reg: u8,
    config_reg: u8,
    read_data_reg: u8,
    read_ptr: u8,
    // Pending second byte expected for multi-byte commands
    pending_cmd: Option<u8>,
    // 1-Wire state machine
    ow_state: OWState,
    // Scratchpad buffer for current conversion
    scratchpad: [u8; 9],
}

impl Ds2482_100 {
    pub fn new() -> Self {
        let mut s = Self {
            temperature_c: 25.0,
            status_reg: STATUS_RST,
            config_reg: 0xE1,
            read_data_reg: 0xFF,
            read_ptr: PTR_STATUS,
            pending_cmd: None,
            ow_state: OWState::Idle,
            scratchpad: [0u8; 9],
        };
        s.build_scratchpad();
        s
    }

    fn build_scratchpad(&mut self) {
        // DS18B20 temperature encoding: 16-bit signed, 1/16 °C per LSB
        let raw = (self.temperature_c * 16.0).round() as i16;
        self.scratchpad[0] = raw as u8;
        self.scratchpad[1] = (raw >> 8) as u8;
        self.scratchpad[2] = 0x00; // TH
        self.scratchpad[3] = 0x00; // TL
        self.scratchpad[4] = 0x7F; // Config (12-bit resolution)
        self.scratchpad[5] = 0xFF;
        self.scratchpad[6] = 0x00;
        self.scratchpad[7] = 0x10;
        self.scratchpad[8] = 0x00; // CRC (not simulated)
    }

    fn current_reg(&self) -> u8 {
        match self.read_ptr {
            PTR_STATUS    => self.status_reg,
            PTR_READ_DATA => self.read_data_reg,
            PTR_CONFIG    => self.config_reg,
            _             => 0xFF,
        }
    }

    fn handle_command_byte(&mut self, byte: u8) -> bool {
        // Returns true if byte was consumed as the second byte of a two-byte command
        if let Some(cmd) = self.pending_cmd.take() {
            match cmd {
                CMD_WRITE_CFG => { self.config_reg = byte; }
                CMD_OW_WRITE  => { self.handle_ow_write(byte); }
                CMD_SET_PTR   => { self.read_ptr = byte; }
                _ => {}
            }
            return true;
        }
        false
    }

    fn handle_ow_write(&mut self, byte: u8) {
        match self.ow_state {
            OWState::Idle => {}
            OWState::RomCmd => {
                if byte == OW_SKIP_ROM {
                    self.ow_state = OWState::ConvertT;
                } else {
                    self.ow_state = OWState::Idle;
                }
            }
            OWState::ConvertT => {
                if byte == OW_CONVERT_T {
                    self.build_scratchpad();
                    self.ow_state = OWState::RomCmd2;
                } else {
                    self.ow_state = OWState::Idle;
                }
            }
            OWState::RomCmd2 => {
                if byte == OW_SKIP_ROM {
                    self.ow_state = OWState::ReadScratchpad { idx: 0 };
                } else {
                    self.ow_state = OWState::Idle;
                }
            }
            OWState::ReadScratchpad { .. } => {
                if byte == OW_READ_SCRATCHPAD {
                    self.ow_state = OWState::ReadScratchpad { idx: 0 };
                } else {
                    self.ow_state = OWState::Idle;
                }
            }
        }
    }

    fn handle_ow_read(&mut self) {
        // Primes read_data_reg with the next scratchpad byte
        if let OWState::ReadScratchpad { ref mut idx } = self.ow_state {
            let i = *idx;
            if i < self.scratchpad.len() {
                self.read_data_reg = self.scratchpad[i];
                *idx = i + 1;
            } else {
                self.read_data_reg = 0xFF;
            }
        } else {
            self.read_data_reg = 0xFF;
        }
        self.read_ptr = PTR_READ_DATA;
    }

    fn process_write(&mut self, data: &[u8]) {
        let mut iter = data.iter().copied();
        while let Some(byte) = iter.next() {
            if self.handle_command_byte(byte) {
                continue;
            }
            match byte {
                CMD_RESET => {
                    self.status_reg = STATUS_RST;
                    self.read_ptr = PTR_STATUS;
                    self.ow_state = OWState::Idle;
                    self.pending_cmd = None;
                }
                CMD_WRITE_CFG | CMD_SET_PTR => {
                    self.pending_cmd = Some(byte);
                }
                CMD_OW_RESET => {
                    self.status_reg = STATUS_PPD;
                    self.read_ptr = PTR_STATUS;
                    self.ow_state = OWState::RomCmd;
                }
                CMD_OW_WRITE => {
                    // Next byte is the 1-Wire data byte
                    self.pending_cmd = Some(CMD_OW_WRITE);
                    // If next byte is embedded in same write, consume it
                    if let Some(data_byte) = iter.next() {
                        self.handle_ow_write(data_byte);
                        self.pending_cmd = None;
                    }
                }
                CMD_OW_READ => {
                    self.handle_ow_read();
                }
                _ => {}
            }
        }
    }
}

impl Default for Ds2482_100 {
    fn default() -> Self { Self::new() }
}

impl IcBehavior for Ds2482_100 {
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
                self.temperature_c = v;
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
            BusTransaction::I2cWrite { data, .. } => {
                self.process_write(data);
                Ok(BusResponse::None)
            }

            BusTransaction::I2cRead { length, .. } => {
                let byte = self.current_reg();
                Ok(BusResponse::Data(vec![byte; length.min(1)]))
            }

            BusTransaction::I2cWriteRead { write, read_length, .. } => {
                self.process_write(write);
                let byte = self.current_reg();
                Ok(BusResponse::Data(vec![byte; read_length.min(1)]))
            }

            _ => Err(IcError::Unsupported("ds2482_100: unsupported transaction")),
        }
    }
}
