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

pub mod ads7961;
pub mod afbr710smz;
pub mod bme280;
pub mod gpio_led;
pub mod kj4b;
pub mod kq5100;
pub mod l6360;
pub mod lm73;
pub mod mc33879;
pub mod mcp23017;
pub mod pca9539a;
pub mod pca9546a;
pub mod pump;
pub mod sq619;
pub mod uart_terminal;

use sim_core::IcBehavior;

/// Construct a fresh `IcBehavior` instance by manifest behavior key
/// (the part after `builtin:`).
///
/// Returns `None` if no behavior is registered under that key.
pub fn registry(key: &str) -> Option<Box<dyn IcBehavior>> {
    match key {
        "ads7961"       => Some(Box::new(ads7961::Ads7961::new())),
        "afbr710smz"    => Some(Box::new(afbr710smz::Afbr710Smz::new())),
        "bme280"        => Some(Box::new(bme280::Bme280::new())),
        "gpio_led"      => Some(Box::new(gpio_led::GpioLed::new())),
        "kj4b"          => Some(Box::new(kj4b::Kj4b::new())),
        "kq5100"        => Some(Box::new(kq5100::Kq5100::new())),
        "l6360"         => Some(Box::new(l6360::L6360::new())),
        "lm73"          => Some(Box::new(lm73::Lm73::new())),
        "mc33879"       => Some(Box::new(mc33879::Mc33879::new())),
        "mcp23017"      => Some(Box::new(mcp23017::Mcp23017::new())),
        "pca9539a"      => Some(Box::new(pca9539a::Pca9539a::new())),
        "pca9546a"      => Some(Box::new(pca9546a::Pca9546a::new())),
        "pump"          => Some(Box::new(pump::Pump::new())),
        "sq619"         => Some(Box::new(sq619::Sq619::new())),
        "uart_terminal" => Some(Box::new(uart_terminal::UartTerminal::new())),
        _ => None,
    }
}
