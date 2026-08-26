use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::Permissions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let before = CargoFilesState::capture(&paths).context("failed to snapshot Cargo files")?;
    let dependency = format!("{SDK_CRATE}@={}", env!("CARGO_PKG_VERSION"));
    let status = Command::new("cargo")
        .args(["add", &dependency, "--package", &package.name])
        .current_dir(workspace_root)
        .status();
    let after = CargoFilesState::capture(&paths)
        .context("failed to observe Cargo files after `cargo add`")?;
    let rollback = CargoRollback {
        paths,
        before,
        after,
    };
    let cargo_result = status
        .context("failed to run `cargo add` for the Provenance SDK")
        .and_then(|status| {
            anyhow::ensure!(
                status.success(),
                "`cargo add {dependency} --package {}` failed",
                package.name
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

struct CargoPaths {
    manifest: PathBuf,
    lock: PathBuf,
}

struct CargoRollback {
    paths: CargoPaths,
    before: CargoFilesState,
    after: CargoFilesState,
}

impl CargoRollback {
    #[rule("rule_cargo_init_restores_owned_files")]
    fn rollback(self) -> anyhow::Result<()> {
        let current = CargoFilesState::capture(&self.paths)
            .context("failed to inspect Cargo files before rollback")?;
        anyhow::ensure!(
            current.equivalent(&self.after),
            "Cargo.toml or Cargo.lock changed after `cargo add`; neither file was restored"
        );

        self.before
            .manifest
            .restore(&self.paths.manifest)
            .with_context(|| format!("failed to restore {}", self.paths.manifest.display()))?;
        if let Err(error) = self.before.lock.restore(&self.paths.lock) {
            return match self.after.manifest.restore(&self.paths.manifest) {
                Ok(()) => Err(error).with_context(|| {
                    format!(
                        "failed to restore {}; restored {} to its post-Cargo state",
                        self.paths.lock.display(),
                        self.paths.manifest.display()
                    )
                }),
                Err(compensation) => Err(error).with_context(|| {
                    format!(
                        "failed to restore {}; also failed to return {} to its post-Cargo state: {compensation}",
                        self.paths.lock.display(),
                        self.paths.manifest.display()
                    )
                }),
            };
        }
        Ok(())
    }
}

struct CargoFilesState {
    manifest: CargoFileState,
    lock: CargoFileState,
}

impl CargoFilesState {
    fn capture(paths: &CargoPaths) -> anyhow::Result<Self> {
        Ok(Self {
            manifest: CargoFileState::capture(&paths.manifest)?,
            lock: CargoFileState::capture(&paths.lock)?,
        })
    }

    fn equivalent(&self, other: &Self) -> bool {
        self.manifest.equivalent(&other.manifest) && self.lock.equivalent(&other.lock)
    }
}

enum CargoFileState {
    Missing,
    Present {
        contents: Vec<u8>,
        permissions: Permissions,
    },
}

impl CargoFileState {
    fn capture(path: &Path) -> anyhow::Result<Self> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::Missing),
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(
            metadata.is_file(),
            "{} is not a regular file",
            path.display()
        );
        Ok(Self::Present {
            contents: std::fs::read(path)?,
            permissions: metadata.permissions(),
        })
    }

    fn equivalent(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Missing, Self::Missing) => true,
            (
                Self::Present {
                    contents: left_contents,
                    permissions: left_permissions,
                },
                Self::Present {
                    contents: right_contents,
                    permissions: right_permissions,
                },
            ) => {
                left_contents == right_contents
                    && permissions_equal(left_permissions, right_permissions)
            }
            _ => false,
        }
    }

    fn restore(self, path: &Path) -> anyhow::Result<()> {
        match self {
            Self::Missing => match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            },
            Self::Present {
                contents,
                permissions,
            } => restore_file(path, &contents, permissions),
        }
    }
}

fn restore_file(path: &Path, contents: &[u8], permissions: Permissions) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for attempt in 0..100_u8 {
        let temporary = parent.join(format!(
            ".{name}.provenance-cargo-rollback-{}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let result = (|| -> std::io::Result<()> {
            file.write_all(contents)?;
            file.sync_all()?;
            std::fs::set_permissions(&temporary, permissions)?;
            crate::atomic_file::replace_path(&temporary, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        return result.map_err(Into::into);
    }
    anyhow::bail!("could not allocate rollback temporary file")
}

#[cfg(unix)]
fn permissions_equal(left: &Permissions, right: &Permissions) -> bool {
    use std::os::unix::fs::PermissionsExt;
    left.mode() == right.mode()
}

#[cfg(not(unix))]
fn permissions_equal(left: &Permissions, right: &Permissions) -> bool {
    left.readonly() == right.readonly()
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
