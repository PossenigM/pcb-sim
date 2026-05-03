//! `sim-bin` — the simulator binary.
//!
//! Wires the simulator's pieces together:
//!   1. Parse CLI args.
//!   2. Initialize logging.
//!   3. Load the IC library and validate it.
//!   4. Load the board YAML and cross-validate against the library.
//!   5. Instantiate the event loop, routers, and behaviors.
//!   6. Connect MQTT (if configured).
//!   7. Bind the IPC sockets for each firmware_host component.
//!   8. Run until SIGINT.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "sim-bin", version, about = "pcb-sim board simulator")]
struct Cli {
    /// Path to the board YAML file.
    #[arg(short, long)]
    board: PathBuf,

    /// Path to the IC library directory.
    #[arg(short, long, default_value = "./ic-library")]
    library: PathBuf,

    /// Override the MQTT broker URL from the board YAML.
    #[arg(long)]
    broker: Option<String>,

    /// Verbosity. Can be repeated: -v info, -vv debug, -vvv trace.
    #[arg(short, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_tracing(cli.verbose);
    tracing::info!(?cli, "starting simulator");

    // TODO: load library, load board, cross-validate.
    // TODO: build event loop and routers.
    // TODO: instantiate behaviors via sim_behaviors::registry.
    // TODO: connect MQTT if configured.
    // TODO: bind IPC sockets for every firmware_host component.
    // TODO: run the event loop.
    // TODO: graceful shutdown on SIGINT.

    let _ = cli;
    eprintln!("sim-bin: not yet implemented. See TODOs in src/main.rs.");
    Ok(())
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| level.to_string());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(false)
        .init();
}
