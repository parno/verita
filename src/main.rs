use crate::config::RunConfiguration;
use anyhow::anyhow;
use clap::Parser as ClapParser;
use regex::Regex;
use std::path::PathBuf;
use toml;
use tracing::{error, info}; // debug, trace
use xshell::{cmd, Shell};

pub mod config;

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

fn get_z3_version(verus_repo: &PathBuf) -> anyhow::Result<String> {
    let sh = Shell::new()?;
    let output = cmd!(sh, "{verus_repo}/source/z3 --version") //.quiet().run()?;
        .output()?;
    dbg!(&output);
    let output_str = String::from_utf8(output.stdout)?;
    let v = Regex::new(r"^Z3 version ([0-9.]*) ")?
        .captures(&output_str)
        .ok_or_else(|| anyhow!("Failed to find Z3 version"))?
        .get(1)
        .expect("missing capture group")
        .as_str()
        .to_string();
    println!("Found Z3 version: {v}");
    Ok(v)
}

fn run(run_configuration_path: &PathBuf) -> Result<(), String> {
    let run_configuration: RunConfiguration = toml::from_str(
        &std::fs::read_to_string(run_configuration_path).map_err(|e| {
            format!(
                "cannot read configuration file {}: {}",
                run_configuration_path.display(),
                e
            )
        })?,
    )
    .map_err(|e| format!("cannot parse run configuration: {}", e))?;
    Ok(())
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
    let _z3_version = get_z3_version(&args.verus_repo);
}
