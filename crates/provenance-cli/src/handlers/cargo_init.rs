use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::HashSet;
use std::process::Command;

mod rollback;
use rollback::{CargoPaths, CargoRollback};

const SDK_CRATE: &str = "provenance-sdk";

pub(super) fn handle(requested_package: Option<&str>) -> anyhow::Result<()> {
    let metadata = load_metadata()?;
    let package = select_package(&metadata, requested_package)?;
    let path_prefix = package_path_prefix(&metadata.workspace_root, package)?;
    ensure_target_unchanged(&metadata, package, &path_prefix)?;

    let init = super::repo::prepare_init(
        &metadata.workspace_root,
        Some("default".to_owned()),
        Some(path_prefix),
        Vec::new(),
        false,
        None,
    )?;
    let cargo_rollback = prepare_sdk(&metadata.workspace_root, package)?;
    if let Err(error) = init.apply() {
        let error = error.context("failed to initialize Provenance state");
        return match cargo_rollback {
            Some(rollback) => match rollback.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(error.context(format!("Cargo rollback failed: {rollback}"))),
            },
            None => Err(error),
        };
    }

    println!(
        "Initialized Provenance {} for Cargo package '{}' in {}.",
        env!("CARGO_PKG_VERSION"),
        package.name,
        metadata.workspace_root
    );
    Ok(())
}

fn ensure_target_unchanged(
    metadata: &CargoMetadata,
    package: &CargoPackage,
    path_prefix: &Utf8Path,
) -> anyhow::Result<()> {
    let Some(existing_prefix) =
        super::repo::scope_path_prefix(&metadata.workspace_root, "default")?
    else {
        return Ok(());
    };
    let existing_path = resolve_scope_path(&metadata.workspace_root, &existing_prefix)?;
    let requested_path = resolve_scope_path(&metadata.workspace_root, path_prefix)?;
    if existing_path == requested_path {
        return Ok(());
    }

    let existing_target = metadata
        .packages
        .iter()
        .find(|candidate| {
            package_path_prefix(&metadata.workspace_root, candidate)
                .and_then(|candidate_prefix| {
                    resolve_scope_path(&metadata.workspace_root, &candidate_prefix)
                })
                .is_ok_and(|candidate_path| candidate_path == existing_path)
        })
        .map_or_else(
            || format!("path prefix '{existing_prefix}'"),
            |candidate| format!("Cargo package '{}'", candidate.name),
        );
    anyhow::bail!(
        "the default Provenance scope already targets {existing_target}; refusing to change it to Cargo package '{}' at path prefix '{}'",
        package.name,
        path_prefix
    )
}

fn resolve_scope_path(
    workspace_root: &Utf8Path,
    prefix: &Utf8Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = if prefix.is_absolute() {
        prefix.to_path_buf()
    } else {
        workspace_root.join(prefix)
    };
    std::fs::canonicalize(&path).with_context(|| {
        format!("failed to resolve Provenance path prefix '{prefix}' from {workspace_root}")
    })
}

fn load_metadata() -> anyhow::Result<CargoMetadata> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .context("failed to run `cargo metadata`; run this command in a Cargo workspace")?;
    anyhow::ensure!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    serde_json::from_slice(&output.stdout).context("`cargo metadata` returned invalid JSON")
}

#[rule("rule_cargo_init_selects_workspace_package")]
fn select_package<'a>(
    metadata: &'a CargoMetadata,
    requested: Option<&str>,
) -> anyhow::Result<&'a CargoPackage> {
    let members: HashSet<_> = metadata.workspace_members.iter().collect();
    let mut eligible: Vec<_> = metadata
        .packages
        .iter()
        .filter(|package| members.contains(&package.id))
        .collect();
    eligible.sort_by(|left, right| left.name.cmp(&right.name));

    if let Some(requested) = requested {
        return eligible
            .into_iter()
            .find(|package| package.name == requested)
            .with_context(|| {
                format!(
                    "Cargo workspace has no eligible package named '{requested}'; available packages: {}",
                    package_names(metadata)
                )
            });
    }

    match eligible.as_slice() {
        [package] => Ok(*package),
        [] => anyhow::bail!("Cargo workspace has no eligible packages"),
        _ => anyhow::bail!(
            "Cargo workspace has more than one eligible package ({}); pass --package <name>",
            eligible
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn package_names(metadata: &CargoMetadata) -> String {
    let members: HashSet<_> = metadata.workspace_members.iter().collect();
    let mut names: Vec<_> = metadata
        .packages
        .iter()
        .filter(|package| members.contains(&package.id))
        .map(|package| package.name.as_str())
        .collect();
    names.sort_unstable();
    names.join(", ")
}

#[rule("rule_cargo_init_uses_package_directory")]
fn package_path_prefix(
    workspace_root: &Utf8Path,
    package: &CargoPackage,
) -> anyhow::Result<Utf8PathBuf> {
    let package_dir = package
        .manifest_path
        .parent()
        .context("selected Cargo package manifest has no parent directory")?;
    let relative = package_dir.strip_prefix(workspace_root).with_context(|| {
        format!(
            "selected package '{}' is outside Cargo workspace {}",
            package.name, workspace_root
        )
    })?;
    Ok(if relative.as_str().is_empty() {
        Utf8PathBuf::from(".")
    } else {
        relative.to_path_buf()
    })
}

/// Adds an absent SDK without replacing a compatible or development declaration.
#[rule("rule_cargo_init_preserves_sdk_dependency")]
#[rule("rule_cargo_init_adds_exact_sdk")]
#[rule("rule_cargo_init_mutates_selected_manifest")]
fn prepare_sdk(
    workspace_root: &Utf8Path,
    package: &CargoPackage,
) -> anyhow::Result<Option<CargoRollback>> {
    let dependencies: Vec<_> = package
        .dependencies
        .iter()
        .filter(|dependency| dependency.name == SDK_CRATE)
        .collect();
    if !dependencies.is_empty() {
        validate_sdk_dependencies(&dependencies)?;
        return Ok(None);
    }

    let paths = CargoPaths {
        manifest: package.manifest_path.as_std_path().to_path_buf(),
        lock: workspace_root.join("Cargo.lock").into_std_path_buf(),
    };
    let rollback = CargoRollback::capture(paths)?;
    let dependency = format!("{SDK_CRATE}@={}", env!("CARGO_PKG_VERSION"));
    let status = Command::new("cargo")
        .args(["add", &dependency, "--manifest-path"])
        .arg(&package.manifest_path)
        .current_dir(workspace_root)
        .status();
    let rollback = rollback.observe_after()?;
    let cargo_result = status
        .context("failed to run `cargo add` for the Provenance SDK")
        .and_then(|status| {
            anyhow::ensure!(
                status.success(),
                "`cargo add {dependency} --manifest-path {}` failed",
                package.manifest_path
            );
            Ok(())
        });
    if let Err(error) = cargo_result {
        return match rollback.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!("Cargo rollback failed: {rollback}"))),
        };
    }
    Ok(Some(rollback))
}

fn validate_sdk_dependencies(dependencies: &[&CargoDependency]) -> anyhow::Result<()> {
    let cli_version = Version::parse(env!("CARGO_PKG_VERSION"))
        .context("the provenance-cli package version is not valid semver")?;
    for dependency in dependencies {
        if dependency.path.is_some()
            || dependency
                .source
                .as_deref()
                .is_some_and(|source| source.starts_with("git+"))
        {
            continue;
        }

        let registry = dependency
            .source
            .as_deref()
            .is_none_or(|source| source.starts_with("registry+") || source.starts_with("sparse+"));
        anyhow::ensure!(
            registry,
            "Cargo package declares {SDK_CRATE} from unsupported source '{}'; preserve it manually or use a path, Git, or registry dependency",
            dependency.source.as_deref().unwrap_or("unknown")
        );
        let requirement = VersionReq::parse(&dependency.req).with_context(|| {
            format!(
                "Cargo returned invalid {SDK_CRATE} version requirement '{}'",
                dependency.req
            )
        })?;
        anyhow::ensure!(
            requirement.matches(&cli_version),
            "Cargo package requires {SDK_CRATE} {}, which is not compatible with provenance-cli {}; refusing to replace the existing declaration",
            dependency.req,
            cli_version
        );
    }
    Ok(())
}

#[derive(Deserialize)]
struct CargoMetadata {
    workspace_root: Utf8PathBuf,
    workspace_members: Vec<String>,
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: Utf8PathBuf,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Deserialize)]
struct CargoDependency {
    name: String,
    req: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    path: Option<Utf8PathBuf>,
}
