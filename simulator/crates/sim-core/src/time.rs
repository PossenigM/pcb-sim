//! Virtual time. Monotonic from simulator start.

use std::time::Duration;

/// Virtual simulator time. Monotonic. Has no relation to wall-clock time
/// except that the event loop generally advances it as wall-clock advances.
pub type SimTime = Duration;
