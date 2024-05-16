use std::path::PathBuf;
use clap::Parser as ClapParser;
use tracing::{error, info}; // debug, trace

#[derive(ClapParser)]
#[command(version, about)]
struct Args {
    /// Base of the Verus repository
    #[arg(short, long)]
    verus_repo: PathBuf,

    /// Path to a run configuration file
    config: PathBuf,

    /// Print debugging output (can be repeated for more detail)
    #[arg(short = 'd', long = "debug", action = clap::ArgAction::Count)]
    debug_level: u8,
}

fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_level(true)
        .with_target(false)
        .with_max_level(match args.debug_level {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            2 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        })
        .init();

    println!("Hello, world!");
}