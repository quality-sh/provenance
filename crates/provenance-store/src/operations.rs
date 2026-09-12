//! Public SDK operation functions.
//!
//! Every operation the wire adapter exposes is reachable here in
//! process. The CLI is a thin argv and stdio adapter over these
//! functions; nothing semantic lives in the adapter.

use anyhow::Context as _;
use std::str::FromStr as _;

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{
    EngineInfo, ScopeId, StableId, SDK_PROTOCOL_VERSION, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

use crate::layout::ProvenanceLayout;
use crate::state_store::{
    BeginVerificationInput, CompleteVerificationInput, StateStore, TypedSpecInput, TypedSpecResult,
};

pub mod catalog;
pub mod files;
mod plan;
pub mod queries;
pub mod read_policy;
pub mod reader;
mod sites;
pub mod stamp;
pub use crate::write_error;

pub use plan::{AffectedRule, ReviewReason, RuleEvidence, TypedSpecPlan};

/// Resolves one explicit root or discovers the nearest enclosing project.
#[rule("rule_sdk_project_discovery")]
pub fn discover_repository(repo: Option<Utf8PathBuf>) -> anyhow::Result<Utf8PathBuf> {
    let cwd = Utf8PathBuf::from_path_buf(std::env::current_dir()?)
        .map_err(|path| anyhow::anyhow!("current directory is not UTF-8: {}", path.display()))?;
    let (start, explicit) = match repo {
        Some(repo) => (
            if repo.is_relative() {
                cwd.join(repo)
            } else {
                repo
            },
            true,
        ),
        None => (cwd, false),
    };
    if explicit {
        return canonical_repository(&start);
    }
    for ancestor in start.ancestors() {
        if ancestor.join(".provenance/state/manifest.json").is_file()
            || ancestor.join(".git").exists()
        {
            return canonical_repository(ancestor);
        }
    }
    canonical_repository(&start)
}

/// Advertises the engine contract. The engine version is the store's own.
pub fn engine_info(repo: Option<Utf8PathBuf>) -> anyhow::Result<EngineInfo> {
    Ok(EngineInfo {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: SDK_PROTOCOL_VERSION,
        state_schema_version: SUPPORTED_SCHEMA_VERSION.0,
        repository: discover_repository(repo)?,
    })
}

/// Calculates one reconciliation, its affected Rules, and their evidence,
/// without writing.
pub fn plan(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    mut input: TypedSpecInput,
) -> anyhow::Result<TypedSpecPlan> {
    let repo = discover_repository(repo)?;
    normalize_implementation_context(&repo, &mut input)?;
    plan::typed_spec(&repo, scope, input)
}

/// Reconciles one language-owned desired-state document with canonical
/// state.
pub fn apply(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    mut input: TypedSpecInput,
) -> anyhow::Result<TypedSpecResult> {
    let repo = discover_repository(repo)?;
    normalize_implementation_context(&repo, &mut input)?;
    StateStore::new(ProvenanceLayout::new(repo)).apply_typed_spec(scope, input)
}

/// Opens one verification run against a durable binding.
///
/// Parses the method word once, here, and carries the typed value through to
/// the store; the word reaches a stored row only through its `Display` form
/// at the run-row edge.
pub fn begin_verification(
    repo: Option<Utf8PathBuf>,
    scope: ScopeId,
    mut input: BeginVerificationInput,
) -> anyhow::Result<provenance_core::VerificationRun> {
    let repo = discover_repository(repo)?;
    let method = provenance_scanner::Verification::from_str(&input.method).map_err(|error| {
        write_error::SourceFailure::wrap(
            write_error::WriteFailure::InvalidVerificationTarget,
            error,
        )
    })?;
    normalize_verification_context(&repo, &mut input)?;
    StateStore::new(ProvenanceLayout::new(repo)).begin_verification(scope, input, method)
}

/// Completes one verification run as passed or failed.
pub fn complete_verification(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    input: CompleteVerificationInput,
) -> anyhow::Result<provenance_core::VerificationRun> {
    let repo = discover_repository(repo)?;
    StateStore::new(ProvenanceLayout::new(repo)).complete_verification(scope, input)
}

/// Lists verification runs, optionally for one Rule.
pub fn verification_runs(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    rule: Option<&StableId>,
) -> anyhow::Result<Vec<provenance_core::VerificationRun>> {
    let repo = discover_repository(repo)?;
    Ok(StateStore::new(ProvenanceLayout::new(repo))
        .list_verification_runs(scope)?
        .into_iter()
        .filter(|run| rule.is_none_or(|rule| &run.rule_id == rule))
        .collect())
}

/// Lists verification bindings, optionally for one Rule.
pub fn verification_bindings(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    rule: Option<&StableId>,
) -> anyhow::Result<Vec<provenance_core::VerificationBinding>> {
    let repo = discover_repository(repo)?;
    Ok(StateStore::new(ProvenanceLayout::new(repo))
        .list_verification_bindings(scope)?
        .into_iter()
        .filter(|binding| rule.is_none_or(|rule| &binding.rule_id == rule))
        .collect())
}

fn canonical_repository(path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| anyhow::anyhow!("resolve repository `{path}`: {error}"))?;
    Utf8PathBuf::from_path_buf(canonical)
        .map_err(|path| anyhow::anyhow!("repository is not UTF-8: {}", path.display()))
}

fn normalize_verification_context(
    repo: &Utf8Path,
    input: &mut BeginVerificationInput,
) -> anyhow::Result<()> {
    let file = input.file.as_ref().ok_or_else(|| {
        write_error::SourceFailure::wrap(
            write_error::WriteFailure::InvalidVerificationTarget,
            anyhow::anyhow!("file is required for a durable verification binding"),
        )
    })?;
    let relative = files::native_relative(repo, file)?;
    let root = files::RepositoryFiles::open(repo)?;
    let _opened = root.open_file(&relative)?;
    input.commit = clean_file_commit(repo, &relative);
    input.file = Some(relative);
    Ok(())
}

/// Resolves each implementation target to one repository file before any
/// store call; a target that does not resolve refuses the operation, so
/// apply writes nothing.
#[rule("rule_rust_store_resolves_implementation_symbols")]
#[rule("rule_rust_symbol_resolution_refuses_writes")]
fn normalize_implementation_context(
    repo: &Utf8Path,
    input: &mut TypedSpecInput,
) -> anyhow::Result<()> {
    for rule in &mut input.rules {
        let Some(implementation) = &mut rule.implementation else {
            continue;
        };
        let relative = files::native_relative(repo, &implementation.file).with_context(|| format!("implementation target `{}` is outside repository `{repo}` or is not a repository path", implementation.file))?;
        let root = files::RepositoryFiles::open(repo)?;
        let _opened = root.open_file(&relative).with_context(|| {
            format!(
                "implementation target `{}` does not exist or cannot be resolved",
                implementation.file
            )
        })?;
        implementation.file = relative;
    }
    Ok(())
}

fn clean_file_commit(repo: &Utf8Path, file: &Utf8Path) -> Option<String> {
    let tracked = crate::stale::git::command(repo)
        .args(["ls-files", "--error-unmatch", "--", file.as_str()])
        .current_dir(repo)
        .output()
        .ok()?;
    if !tracked.status.success() {
        return None;
    }
    let status = crate::stale::git::command(repo)
        .args(["status", "--porcelain", "--", file.as_str()])
        .current_dir(repo)
        .output()
        .ok()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return None;
    }
    let head = crate::stale::git::command(repo)
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()?;
    if !head.status.success() {
        return None;
    }
    Some(String::from_utf8(head.stdout).ok()?.trim().to_string())
}

#[cfg(test)]
mod write_tests;
