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

fn get_solver_version(verus_repo: &PathBuf, solver_exe: &str, fmt_str: &str) -> anyhow::Result<String> {
    let sh = Shell::new()?;
    let output = cmd!(sh, "{verus_repo}/source/{solver_exe} --version") //.quiet().run()?;
        .output()?;
    //dbg!(&output);
    let output_str = String::from_utf8(output.stdout)?;
    let fmt = format!("{fmt_str} ([0-9.]*) ");
    let v = Regex::new(&fmt)?
        .captures(&output_str)
        .ok_or_else(|| anyhow!("Failed to find {solver_exe} version"))?
        .get(1)
        .expect("missing capture group")
        .as_str()
        .to_string();
    println!("Found {solver_exe} version: {v}");
    Ok(v)
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
    let _z3_version = get_solver_version(&args.verus_repo, "z3", "Z3 version");
    let _cvc5_version = get_solver_version(&args.verus_repo, "cvc5", "This is cvc5 version");
}
