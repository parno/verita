use anyhow::anyhow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Scan `{verus_repo}/source/` and return a map from package name to its directory path.
fn build_verus_crate_map(verus_repo: &Path) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let source_dir = verus_repo.join("source");
    let entries = match std::fs::read_dir(&source_dir) {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let cargo_toml_path = entry.path().join("Cargo.toml");
        let content = match std::fs::read_to_string(&cargo_toml_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let manifest: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(name) = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            map.insert(name.to_string(), entry.path());
        }
    }
    map
}

/// Walk up from `target_dir` to `repo_root`, returning the highest ancestor that
/// has a `[workspace]` section in its `Cargo.toml`. Falls back to `target_dir`.
fn find_workspace_root(target_dir: &Path, repo_root: &Path) -> PathBuf {
    let mut ancestors: Vec<PathBuf> = Vec::new();
    let mut current = target_dir.to_path_buf();
    loop {
        ancestors.push(current.clone());
        if current == repo_root {
            break;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => break,
        }
    }
    // Search from repo_root toward target_dir; return first one with [workspace]
    for dir in ancestors.iter().rev() {
        if let Ok(content) = std::fs::read_to_string(dir.join("Cargo.toml")) {
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                if value.get("workspace").is_some() {
                    return dir.clone();
                }
            }
        }
    }
    target_dir.to_path_buf()
}

/// Collect all dependency names declared in a manifest (all dep sections +
/// workspace.dependencies).
fn collect_dep_names(manifest: &toml::Value) -> HashSet<String> {
    let mut names = HashSet::new();
    for section in &["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = manifest.get(section).and_then(|d| d.as_table()) {
            names.extend(deps.keys().cloned());
        }
    }
    if let Some(wdeps) = manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        names.extend(wdeps.keys().cloned());
    }
    names
}

/// If the project's workspace references any Verus crates, add `[patch]` entries
/// to the workspace root `Cargo.toml` so those crates resolve to the local Verus
/// repo rather than whatever version/source the project specified.
///
/// Two patch sources are written – `crates-io` and the Verus git URL – so the
/// override works regardless of how the project declared its dependency.
pub fn inject_verus_patches(
    target_dir: &Path,
    repo_root: &Path,
    verus_repo: &Path,
    verus_git_url: &str,
) -> anyhow::Result<()> {
    let verus_crate_map = build_verus_crate_map(verus_repo);
    if verus_crate_map.is_empty() {
        return Ok(());
    }

    let workspace_root = find_workspace_root(target_dir, repo_root);
    let workspace_cargo_toml = workspace_root.join("Cargo.toml");

    // Collect dep names from both the workspace root and the target crate
    let mut all_dep_names: HashSet<String> = HashSet::new();
    for path in [&workspace_cargo_toml, &target_dir.join("Cargo.toml")] {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(manifest) = toml::from_str::<toml::Value>(&content) {
                all_dep_names.extend(collect_dep_names(&manifest));
            }
        }
    }

    // Filter to only Verus crates that the project actually references
    let patches: Vec<(String, PathBuf)> = verus_crate_map
        .into_iter()
        .filter(|(name, _)| all_dep_names.contains(name))
        .collect();

    if patches.is_empty() {
        return Ok(());
    }

    debug!(
        "Injecting path patches for {} Verus crate(s) into {}",
        patches.len(),
        workspace_cargo_toml.display()
    );

    let content = std::fs::read_to_string(&workspace_cargo_toml)
        .map_err(|e| anyhow!("cannot read {}: {}", workspace_cargo_toml.display(), e))?;
    let mut manifest: toml::Value = toml::from_str(&content)
        .map_err(|e| anyhow!("cannot parse {}: {}", workspace_cargo_toml.display(), e))?;

    // Build the table of { crate_name = { path = "..." } } entries
    let mut patch_entries = toml::map::Map::new();
    for (crate_name, crate_path) in &patches {
        let mut entry = toml::map::Map::new();
        entry.insert(
            "path".to_string(),
            toml::Value::String(crate_path.to_string_lossy().into_owned()),
        );
        patch_entries.insert(crate_name.clone(), toml::Value::Table(entry));
        debug!("  {} -> {}", crate_name, crate_path.display());
    }

    // Ensure [patch] table exists
    if manifest.get("patch").is_none() {
        manifest
            .as_table_mut()
            .ok_or_else(|| anyhow!("manifest root is not a table"))?
            .insert("patch".to_string(), toml::Value::Table(toml::map::Map::new()));
    }
    let patch_table = manifest["patch"]
        .as_table_mut()
        .ok_or_else(|| anyhow!("[patch] is not a table"))?;

    // Merge into [patch.crates-io]
    match patch_table.get_mut("crates-io") {
        Some(toml::Value::Table(t)) => {
            for (k, v) in &patch_entries {
                t.insert(k.clone(), v.clone());
            }
        }
        _ => {
            patch_table
                .insert("crates-io".to_string(), toml::Value::Table(patch_entries.clone()));
        }
    }

    // Merge into [patch."<verus_git_url>"]
    match patch_table.get_mut(verus_git_url) {
        Some(toml::Value::Table(t)) => {
            for (k, v) in &patch_entries {
                t.insert(k.clone(), v.clone());
            }
        }
        _ => {
            patch_table.insert(verus_git_url.to_string(), toml::Value::Table(patch_entries));
        }
    }

    let new_content = toml::to_string_pretty(&manifest)
        .map_err(|e| anyhow!("cannot serialize {}: {}", workspace_cargo_toml.display(), e))?;
    std::fs::write(&workspace_cargo_toml, new_content)
        .map_err(|e| anyhow!("cannot write {}: {}", workspace_cargo_toml.display(), e))?;

    Ok(())
}
