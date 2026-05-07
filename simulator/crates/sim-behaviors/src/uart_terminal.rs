//! UART terminal behavior backed by a host PTY.
//!
//! On Linux the slave end of the PTY appears as `/dev/pts/N`, which any
//! serial terminal (picocom, screen, minicom) can open directly.
//!
//! Data flow:
//!   firmware → on_bus_transaction → write to PTY master
//!   PTY master read in on_tick   → bus_transmit → firmware

use sim_core::{
    BusId, BusResponse, BusTransaction, IcBehavior, IcError, InitCtx, OwnedBusTransaction, RunCtx,
    SimTime,
};
use std::ffi::CStr;
use std::os::unix::io::{FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(1);
const READ_BUF: usize = 256;

pub struct UartTerminal {
    master_fd: RawFd,
    // Held open so master never gets ENXIO when no client is connected yet.
    _slave_fd: Option<OwnedFd>,
    bus_id: Option<BusId>,
}

impl UartTerminal {
    pub fn new() -> Self {
        Self { master_fd: -1, _slave_fd: None, bus_id: None }
    }
}

impl Default for UartTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for UartTerminal {
    fn drop(&mut self) {
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd) };
        }
    }
}

impl IcBehavior for UartTerminal {
    fn init(&mut self, ctx: &mut InitCtx<'_>) -> Result<(), IcError> {
        let (master_fd, slave_fd) = open_pty()
            .map_err(|e| IcError::Internal(format!("uart_terminal: PTY open failed: {e}")))?;

        let slave_path = unsafe {
            let ptr = libc::ptsname(master_fd);
            if ptr.is_null() {
                "<unknown>".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };

        tracing::info!("uart_terminal: PTY ready — connect with:  picocom {slave_path}");

        self.bus_id = ctx.bus_id("uart");
        self.master_fd = master_fd;
        // SAFETY: slave_fd is a valid, owned FD from open_pty.
        self._slave_fd = Some(unsafe { OwnedFd::from_raw_fd(slave_fd) });

        ctx.schedule_tick(POLL_INTERVAL);
        Ok(())
    }

    fn on_bus_transaction(
        &mut self,
        txn: BusTransaction<'_>,
        _ctx: &mut RunCtx<'_>,
    ) -> Result<BusResponse, IcError> {
        match txn {
            BusTransaction::UartFrame { data } => {
                if self.master_fd >= 0 && !data.is_empty() {
                    unsafe {
                        libc::write(
                            self.master_fd,
                            data.as_ptr() as *const libc::c_void,
                            data.len(),
                        );
                    }
                }
                Ok(BusResponse::None)
            }
            _ => Err(IcError::Unsupported("uart_terminal: not a UART bus")),
        }
    }

    fn on_tick(&mut self, _now: SimTime, ctx: &mut RunCtx<'_>) -> Result<(), IcError> {
        if self.master_fd >= 0 {
            if let Some(bus_id) = self.bus_id {
                let mut buf = [0u8; READ_BUF];
                loop {
                    let n = unsafe {
                        libc::read(
                            self.master_fd,
                            buf.as_mut_ptr() as *mut libc::c_void,
                            buf.len(),
                        )
                    };
                    if n > 0 {
                        ctx.bus_transmit(
                            bus_id,
                            OwnedBusTransaction::UartFrame { data: buf[..n as usize].to_vec() },
                        );
                    } else {
                        // n == 0 (EOF) or n < 0 (EAGAIN / error) — stop polling.
                        break;
                    }
                }
            }
        }
        ctx.schedule_tick(POLL_INTERVAL);
        Ok(())
    }
}

/// Create a PTY pair. Returns `(master_fd, slave_fd)` with `O_NONBLOCK` on master.
///
/// The slave is configured in raw mode so that bytes written to the master
/// (firmware TX) are not echoed back into the master read buffer (which would
/// be delivered to the firmware as spurious RX data).
fn open_pty() -> Result<(RawFd, RawFd), String> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;

    let ret = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }

    // Set master non-blocking so on_tick reads return immediately.
    let flags = unsafe { libc::fcntl(master, libc::F_GETFL) };
    if flags < 0 {
        unsafe { libc::close(master); libc::close(slave) };
        return Err(std::io::Error::last_os_error().to_string());
    }
    let ret = unsafe { libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if ret < 0 {
        unsafe { libc::close(master); libc::close(slave) };
        return Err(std::io::Error::last_os_error().to_string());
    }

    // Disable echo and all other line-discipline processing on the slave so
    // that bytes written to the master (firmware TX) are not echoed back into
    // the master read buffer and mistakenly fed to the firmware as RX data.
    let mut tios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(slave, &mut tios) } == 0 {
        unsafe { libc::cfmakeraw(&mut tios) };
        unsafe { libc::tcsetattr(slave, libc::TCSANOW, &tios) };
    }

    Ok((master, slave))
}
