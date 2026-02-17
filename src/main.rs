use crate::config::RunConfiguration;
use crate::config::RunConfigurationProject;
use crate::output::VerusOutput;
use anyhow::anyhow;
use clap::Parser as ClapParser;
use git2::Repository;
use regex::Regex;
use std::{fs, path::Path, path::PathBuf};
use tempdir::TempDir;
use tracing::{error, info, warn}; // debug, trace
use xshell::{cmd, Shell};

pub mod config;
pub mod output;

#[derive(ClapParser)]
#[command(version, about)]
struct Args {
    /// Base of the Verus repository
    #[arg(short, long)]
    verus_repo: PathBuf,
    /// Path to the Singular algebra solver
    #[arg(short, long)]
    singular: Option<PathBuf>,
    /// Path to a run configuration file
    config: PathBuf,
    /// Label for the run
    #[arg(short, long)]
    label: String,
    /// Print debugging output (can be repeated for more detail).
    /// This will also cause verita to retain the project repos it clones.
    #[arg(short = 'd', long = "debug", action = clap::ArgAction::Count)]
    debug_level: u8,
}

fn get_solver_version(
    verus_repo: &PathBuf,
    solver_exe: &str,
    fmt_str: &str,
) -> anyhow::Result<String> {
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

pub fn log_command(cmd: std::process::Command) -> std::process::Command {
    info!("running: {:?}", &cmd);
    cmd
}

type ProjectSummary = (
    RunConfigurationProject,
    bool,
    String,
    std::time::Duration,
    Option<VerusOutput>,
);

/// Shared context for processing projects and targets within a run.
struct RunContext<'a> {
    sh: &'a Shell,
    verus_binary_path: &'a Path,
    cargo_verus_binary_path: &'a Path,
    run_configuration: &'a RunConfiguration,
    output_path: &'a Path,
    label: &'a str,
    date: &'a str,
    z3_version: &'a str,
    cvc5_version: &'a str,
}

/// Process a single crate root target within a project.
/// Returns the summary tuple for this target, or an error.
fn process_target(
    ctx: &RunContext,
    project: &RunConfigurationProject,
    repo_path: &Path,
    target: &str,
    target_index: usize,
    total_targets: usize,
    hash: &str,
) -> anyhow::Result<(ProjectSummary, bool)> {
    let sh = ctx.sh;
    let verus_binary_path = ctx.verus_binary_path;
    let cargo_verus_binary_path = ctx.cargo_verus_binary_path;

    info!(
        "running target {target} ({} of {})",
        target_index + 1,
        total_targets
    );
    let project_verification_start = std::time::Instant::now();
    let output = if project.cargo_verus {
        // Run cargo-verus verify in the target directory
        sh.change_dir(repo_path.join(target));
        log_command(
            cmd!(
                sh,
                "{cargo_verus_binary_path} verus verify -- --output-json --time"
            )
            .args(ctx.run_configuration.verus_extra_args.iter().flatten())
            .args(project.extra_args.iter().flatten())
            .into(),
        )
        .output()
        .map_err(|e| anyhow!("cannot execute cargo verus on {}: {}", &project.name, e))?
    } else {
        log_command(
            cmd!(
                sh,
                "{verus_binary_path} --output-json --time {target}"
            )
            .args(ctx.run_configuration.verus_extra_args.iter().flatten())
            .args(project.extra_args.iter().flatten())
            .into(),
        )
        .output()
        .map_err(|e| anyhow!("cannot execute verus on {}: {}", &project.name, e))?
    };
    let project_verification_duration = project_verification_start.elapsed();

    let verus_failed = !output.status.success();

    // Build output filename: use project name alone for single targets,
    // or "project-crate-root-dir" for multiple targets
    let output_name = if total_targets == 1 {
        project.name.clone()
    } else {
        let dir = Path::new(target)
            .parent()
            .map(|p| p.to_string_lossy().replace('/', "-"))
            .unwrap_or_default();
        if dir.is_empty() {
            project.name.clone()
        } else {
            format!("{}-{}", project.name, dir)
        }
    };
    let project_output_path_json = ctx.output_path.join(&output_name).with_extension("json");

    let (output_json, verus_output) =
        match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            Ok(mut output_json) => {
                let verus_output: Option<VerusOutput> =
                    match serde_json::from_value(output_json.clone()) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            error!(
                                "cannot parse verus json output for {}: {}",
                                &project.name, e
                            );
                            error!("got: {:?}", output_json);
                            None
                        }
                    };
                let duration_ms_value = serde_json::Value::Number(
                    serde_json::Number::from_f64(
                        project_verification_duration.as_millis() as f64,
                    )
                    .expect("valid verus_build_duration"),
                );
                output_json["runner"] = serde_json::json!({
                    "success": output.status.success(),
                    "stderr": String::from_utf8_lossy(&output.stderr),
                    "verus_git_url": ctx.run_configuration.verus_git_url,
                    "verus_refspec": ctx.run_configuration.verus_refspec,
                    "verus_features": ctx.run_configuration.verus_features,
                    "run_configuration": project,
                    "verification_duration_ms": duration_ms_value,
                    "z3_version": ctx.z3_version,
                    "cvc5_version": ctx.cvc5_version,
                    "label": ctx.label,
                    "date": ctx.date,
                });
                (output_json, verus_output)
            }
            Err(e) => {
                error!("cannot parse verus output for {}: {}", &project.name, e);
                error!("got: {}", &String::from_utf8(output.stdout)?);
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
        serde_json::to_string_pretty(&output_json)?,
    )
    .map_err(|e| anyhow!("cannot write output json: {}", e))?;

    Ok((
        (
            project.clone(),
            output.status.success(),
            hash.to_string(),
            project_verification_duration,
            verus_output,
        ),
        verus_failed,
    ))
}

/// Process a single project: clone, checkout, prepare, and run all crate root targets.
/// Returns a list of per-target summaries and whether any target had Verus failures.
fn process_project(
    ctx: &RunContext,
    project: &RunConfigurationProject,
    workdir: &Path,
) -> anyhow::Result<(Vec<ProjectSummary>, bool)> {
    info!("running project {}", project.name);

    info!("\tCloning project");
    let repo_path = workdir.join(&project.name);
    let project_repo = Repository::clone(&project.git_url, &repo_path)?;
    let (rev, _reference) = project_repo
        .revparse_ext(&project.refspec)
        .map_err(|e| anyhow!("failed to find {}: {}", project.refspec, e))?;
    project_repo.checkout_tree(&rev, None)?;
    let hash = rev.id().to_string();
    ctx.sh.change_dir(&repo_path);

    if let Some(prepare_script) = &project.prepare_script {
        log_command(cmd!(ctx.sh, "/bin/bash -c {prepare_script}").into())
            .status()
            .map_err(|e| {
                anyhow!("cannot execute prepare script for {}: {}", &project.name, e)
            })?;
    }

    let mut summaries = Vec::new();
    let mut any_verus_failure = false;

    for (target_index, target) in project.crate_roots.iter().enumerate() {
        match process_target(
            ctx,
            project,
            &repo_path,
            target,
            target_index,
            project.crate_roots.len(),
            &hash,
        ) {
            Ok((summary, verus_failed)) => {
                if verus_failed {
                    any_verus_failure = true;
                }
                summaries.push(summary);
            }
            Err(e) => {
                error!(
                    "Failed to process target {} for project {}: {}",
                    target, project.name, e
                );
                // Write an error JSON so the failure is recorded in output
                let output_name = if project.crate_roots.len() == 1 {
                    project.name.clone()
                } else {
                    let dir = Path::new(target)
                        .parent()
                        .map(|p| p.to_string_lossy().replace('/', "-"))
                        .unwrap_or_default();
                    if dir.is_empty() {
                        project.name.clone()
                    } else {
                        format!("{}-{}", project.name, dir)
                    }
                };
                let error_json = serde_json::json!({
                    "runner": {
                        "success": false,
                        "error": format!("{}", e),
                        "run_configuration": project,
                    }
                });
                let error_path = ctx.output_path.join(&output_name).with_extension("json");
                if let Err(write_err) = std::fs::write(
                    &error_path,
                    serde_json::to_string_pretty(&error_json).unwrap(),
                ) {
                    error!(
                        "Failed to write error json for {} target {}: {}",
                        project.name, target, write_err
                    );
                }
                any_verus_failure = true;
            }
        }
    }

    Ok((summaries, any_verus_failure))
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
    let verus_repo = std::fs::canonicalize(args.verus_repo)?;

    let z3_version = match get_solver_version(&verus_repo, "z3", "Z3 version") {
        Ok(v) => v,
        Err(_) => "unknown".to_string(),
    };
    let cvc5_version = match get_solver_version(&verus_repo, "cvc5", "This is cvc5 version") {
        Ok(v) => v,
        Err(_) => "unknown".to_string(),
    };

    // let verus_repo = Repository::open(args.verus_repo)?;
    // println!("Found repo with head {:?}, state {:?}, ", verus_repo.head()?.name().unwrap(), verus_repo.state());

    // Check that verus executable is present
    let verus_binary_path = verus_repo.join("source/target-verus/release/verus");
    if fs::metadata(&verus_binary_path).is_err() {
        return Err(anyhow!(
            "failed to find verus binary: {}",
            verus_binary_path.display()
        ));
    }
    info!("Found verus binary");

    let cargo_verus_binary_path = verus_repo.join("source/target-verus/release/cargo-verus");

    let run_configuration: RunConfiguration =
        toml::from_str(&std::fs::read_to_string(&args.config).map_err(|e| {
            anyhow!(
                "cannot read configuration file {}: {}",
                args.config.display(),
                e
            )
        })?)
        .map_err(|e| anyhow!("cannot parse run configuration: {}", e))?;

    info!("Loaded run configuration:");
    dbg!(&run_configuration);

    // Check that cargo-verus executable is present if any project needs it
    if run_configuration.projects.iter().any(|p| p.cargo_verus) {
        if fs::metadata(&cargo_verus_binary_path).is_err() {
            return Err(anyhow!(
                "failed to find cargo-verus binary: {}",
                cargo_verus_binary_path.display()
            ));
        }
        info!("Found cargo-verus binary");
    }

    info!("Running projects");
    let sh = Shell::new()?;
    sh.set_var("VERUS_Z3_PATH", verus_repo.join("source/z3"));
    sh.set_var("VERUS_CVC5_PATH", verus_repo.join("source/cvc5"));

    // If the Singular option is provided, confirm the binary exists and set the environment variable
    if let Some(p) = args.singular {
        if fs::metadata(&p).is_err() {
            return Err(anyhow!(
                "failed to find specified Singular binary: {}",
                p.display()
            ));
        }
        sh.set_var("VERUS_SINGULAR_PATH", p);
    }

    let date = chrono::Utc::now()
        .format("%Y-%m-%d-%H-%M-%S-%3f")
        .to_string();
    let output_path = Path::new("output").join(format!("{}-{}", &date, &args.label));
    let tmp_dir = TempDir::new("verita")?;
    let perm_temp_dir = std::env::temp_dir().join("verita").join(&date);
    std::fs::create_dir_all(&output_path)?;
    let workdir = if args.debug_level > 0 {
        // Use a directory that won't disappear after we run, so we can debug any issues that arise
        perm_temp_dir.as_path()
    } else {
        // Use a directory that will be automatically reclaimed after we terminate
        tmp_dir.path()
    };
    dbg!(&workdir);

    let ctx = RunContext {
        sh: &sh,
        verus_binary_path: &verus_binary_path,
        cargo_verus_binary_path: &cargo_verus_binary_path,
        run_configuration: &run_configuration,
        output_path: &output_path,
        label: &args.label,
        date: &date,
        z3_version: &z3_version,
        cvc5_version: &cvc5_version,
    };

    let mut project_summaries = Vec::new();
    let mut failed_projects: Vec<String> = Vec::new();
    let mut succeeded_projects: Vec<String> = Vec::new();

    for project in run_configuration.projects.iter() {
        match process_project(&ctx, project, workdir) {
            Ok((summaries, any_verus_failure)) => {
                // If any target had Verus failures and we're in auto-cleanup mode,
                // preserve the repo for debugging
                if any_verus_failure && args.debug_level == 0 {
                    let src = workdir.join(&project.name);
                    let dest = perm_temp_dir.join(&project.name);
                    if src.exists() {
                        if let Err(e) = fs::create_dir_all(&perm_temp_dir) {
                            warn!(
                                "Failed to create persistent directory for {}: {}",
                                project.name, e
                            );
                        } else {
                            // Use a rename if possible (same filesystem), otherwise warn
                            if let Err(e) = fs::rename(&src, &dest) {
                                warn!(
                                    "Failed to preserve repo for {} (rename failed: {})",
                                    project.name, e
                                );
                            } else {
                                println!(
                                    "Preserved repo for {} (had verification errors) at: {}",
                                    project.name,
                                    dest.display()
                                );
                            }
                        }
                    }
                }

                if any_verus_failure {
                    failed_projects.push(project.name.clone());
                } else {
                    succeeded_projects.push(project.name.clone());
                }
                project_summaries.extend(summaries);
            }
            Err(e) => {
                error!("Failed to process project {}: {}", project.name, e);
                failed_projects.push(project.name.clone());
                // Write an error JSON so the failure is recorded in output
                let error_json = serde_json::json!({
                    "runner": {
                        "success": false,
                        "error": format!("{}", e),
                        "run_configuration": project,
                    }
                });
                let error_path = output_path.join(&project.name).with_extension("json");
                if let Err(write_err) = std::fs::write(
                    &error_path,
                    serde_json::to_string_pretty(&error_json).unwrap(),
                ) {
                    error!(
                        "Failed to write error json for {}: {}",
                        project.name, write_err
                    );
                }
            }
        }
    }

    // Print summary
    println!("\n--- Run Summary ---");
    println!(
        "Total projects: {}",
        succeeded_projects.len() + failed_projects.len()
    );
    if !succeeded_projects.is_empty() {
        println!("Succeeded ({}): {}", succeeded_projects.len(), succeeded_projects.join(", "));
    }
    if !failed_projects.is_empty() {
        println!("Failed ({}): {}", failed_projects.len(), failed_projects.join(", "));
    }
    println!("Output: {}", output_path.display());

    Ok(())
}
