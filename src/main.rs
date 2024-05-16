use crate::config::RunConfiguration;
use anyhow::anyhow;
use clap::Parser as ClapParser;
use git2::Repository;
use regex::Regex;
use std::{fs, path::PathBuf};
use tempdir::TempDir;
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

pub fn log_command(cmd: &mut std::process::Command) -> &mut std::process::Command {
    info!("running: {:?}", &cmd);
    cmd
}

fn main() -> anyhow::Result<()> {
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
//    let _z3_version = get_solver_version(&args.verus_repo, "z3", "Z3 version");
//   let _cvc5_version = get_solver_version(&args.verus_repo, "cvc5", "This is cvc5 version");

    // let verus_repo = Repository::open(args.verus_repo)?;
    // println!("Found repo with head {:?}, state {:?}, ", verus_repo.head()?.name().unwrap(), verus_repo.state());

    // Check that verus executable is present
    let verus_binary_path = args.verus_repo.join("source/target-verus/release/verus");
    if !fs::metadata(&verus_binary_path).is_ok() {
        return Err(anyhow!("failed to find verus binary: {}", verus_binary_path.display()));
    }
    info!("Found verus binary");

    let run_configuration: RunConfiguration = toml::from_str(
            &std::fs::read_to_string(&args.config).map_err(|e| {
                anyhow!(
                    "cannot read configuration file {}: {}",
                    args.config.display(),
                    e
                )
            })?,
        )
        .map_err(|e| anyhow!("cannot parse run configuration: {}", e))?;

    info!("Loaded run configuration:");
    dbg!(&run_configuration);

    info!("Running projects");
    let sh = Shell::new()?;
    sh.set_var("VERUS_Z3_PATH", args.verus_repo.join("source/z3"));
    sh.set_var("VERUS_CVC5_PATH", args.verus_repo.join("source/cvc5"));

//    let workdir = TempDir::new("verita")?;
    let date = chrono::Utc::now().format("%Y-%m-%d-%H-%M-%S-%3f").to_string();
    let workdir =  std::env::temp_dir().join("verita").join(date);
    //let mut project_summaries = Vec::new();
    for project in run_configuration.projects.iter() {
        info!("running project {}", project.name);

        info!("\tCloning project");
        //let repo_path = workdir.path().join(&project.name);
        let repo_path = workdir.join(&project.name);
        let project_repo = Repository::clone(&project.git_url, &repo_path)?;
        let (rev, _reference) = project_repo.revparse_ext(&project.refspec)
        .map_err(|e| anyhow!("failed to find {}: {}", project.refspec, e))?;        
        project_repo.checkout_tree(&rev, None);
        sh.change_dir(repo_path);

        if let Some(prepare_script) = &project.prepare_script {
            let result = log_command(&mut cmd!(sh, "/bin/bash -c {prepare_script}").into())
            .status()
            .map_err(|e| anyhow!("cannot execute prepare script for {}: {}", &project.name, e))?;
            //result.success_or_err()?;
        }
        let project_verification_start = std::time::Instant::now();
        let target = &project.crate_root;
        let output = log_command(
            &mut cmd!(sh, "{verus_binary_path} --output-json --time --no-report-long-running {target}").into() 
        )
        .args(run_configuration.verus_extra_args.iter().flatten())
        .args(project.extra_args.iter().flatten())
        .output()
        .map_err(|e| anyhow!("cannot execute verus on {}: {}", &project.name, e))?;
        dbg!(output);
        let project_verification_duration = project_verification_start.elapsed();
/* 
        let project_run_configuration_digest = digest::obj_digest(&project);
        let project_output_path_json = run_output_path
            .join(project.name.to_owned() + "-" + &project_run_configuration_digest)
            .with_extension("json");

        let (output_json, verus_output) =
            match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                Ok(mut output_json) => {
                    let verus_output: Option<VerusOutput> =
                        match serde_json::from_value(output_json.clone()) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                warn(&format!(
                                    "cannot parse verus output for {}: {}",
                                    &project.name, e
                                ));
                                None
                            }
                        };
                    let duration_ms_value = serde_json::Value::Number(
                        serde_json::Number::from_f64(
                            project_verification_duration.as_millis() as f64
                        )
                        .expect("valid verus_build_duration"),
                    );
                    output_json["runner"] = serde_json::json!({
                        "success": output.status.success(),
                        "stderr": String::from_utf8_lossy(&output.stderr),
                        "verus_git_url": run_configuration.verus_git_url,
                        "verus_refspec": run_configuration.verus_refspec,
                        "verus_features": run_configuration.verus_features,
                        "run_configuration": project,
                        "verification_duration_ms": duration_ms_value,
                    });
                    (output_json, verus_output)
                }
                Err(e) => {
                    warn(&format!(
                        "cannot parse verus output for {}: {}",
                        &project.name, e
                    ));
                    (
                        serde_json::json!({
                            "runner": {
                                "success": output.status.success(),
                                "stderr": String::from_utf8_lossy(&output.stderr),
                                "invalid_output_json": true,
                            }
                        }),
                        None,
                    )
                }
            };
        std::fs::write(
            &project_output_path_json,
            serde_json::to_string_pretty(&output_json).unwrap(),
        )
        .map_err(|e| format!("cannot write output json: {}", e))?;

        project_summaries.push((
            project.clone(),
            output.status.success(),
            proj_checkout.hash,
            project_verification_duration,
            verus_output,
        ));
        */
    }

    // For each project, create a temporary directory, checkout the repo, and execute stuff
    Ok(())
}
