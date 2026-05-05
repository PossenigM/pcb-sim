//! `sim-behaviors` — built-in IC behavior implementations.
//!
//! Each module implements `IcBehavior` for one IC type. The `registry`
//! function maps the manifest's `behavior: builtin:<name>` string to a
//! constructor.
//!
//! ## Adding a new behavior
//!
//! 1. Create `<ic_name>.rs` and implement `IcBehavior`.
//! 2. Add a `pub mod <ic_name>;` line below.
//! 3. Add an arm to `registry()` matching the manifest's behavior key.
//! 4. Write a unit test using a mock `RunCtx`.

pub mod bme280;
pub mod gpio_led;
pub mod mcp23017;

use sim_core::IcBehavior;

/// Construct a fresh `IcBehavior` instance by manifest behavior key
/// (the part after `builtin:`).
///
/// Returns `None` if no behavior is registered under that key.
pub fn registry(key: &str) -> Option<Box<dyn IcBehavior>> {
    match key {
        "bme280"   => Some(Box::new(bme280::Bme280::new())),
        "mcp23017" => Some(Box::new(mcp23017::Mcp23017::new())),
        "gpio_led" => Some(Box::new(gpio_led::GpioLed::new())),
        _ => None,
    }
}
