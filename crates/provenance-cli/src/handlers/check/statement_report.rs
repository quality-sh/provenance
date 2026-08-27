use anyhow::Context;
use camino::Utf8Path;
use provenance_core::{Manifest, Requirement, Rule, Scope, ScopeId};
use provenance_macros::rule;
use provenance_store::{
    state_store::StateStore,
    statement_analysis::{analyze_changed_statements, StatementDiagnostic},
};
use serde::de::DeserializeOwned;

pub(super) struct CommittedStatementAnalysis {
    pub candidate_commit: String,
    pub base_commit: Option<String>,
    pub diagnostics: Vec<StatementDiagnostic>,
}

#[derive(Default)]
struct StatementFamily {
    requirements: Vec<Requirement>,
    rules: Vec<Rule>,
}

#[rule("rule_ste_manual_changed_statement_report")]
pub(super) fn changed_statements_from_head(
    store: &StateStore,
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<Vec<StatementDiagnostic>> {
    if !has_head(repo)? {
        return Ok(Vec::new());
    }

    let base = read_commit_family(repo, "HEAD", &manifest.scopes)?;
    let candidate = read_working_family(store, &manifest.scopes)?;
    Ok(analyze_family_change(repo, &base, &candidate))
}

#[rule("rule_ste_strict_committed_statement_selection")]
pub(super) fn changed_statements_from_commits(
    repo: &Utf8Path,
    manifest: &Manifest,
    explicit_base: Option<&str>,
) -> anyhow::Result<CommittedStatementAnalysis> {
    let candidate_commit = resolve_commit(repo, "HEAD")?;
    let base_commit = match explicit_base {
        Some(base) => Some(resolve_commit(repo, base)?),
        None => first_parent(repo, &candidate_commit)?,
    };
    let base = base_commit.as_deref().map_or_else(
        || Ok(StatementFamily::default()),
        |base| read_commit_family(repo, base, &manifest.scopes),
    )?;
    let candidate = read_commit_family(repo, &candidate_commit, &manifest.scopes)?;
    let diagnostics = analyze_family_change(repo, &base, &candidate);
    Ok(CommittedStatementAnalysis {
        candidate_commit,
        base_commit,
        diagnostics,
    })
}

fn has_head(repo: &Utf8Path) -> anyhow::Result<bool> {
    Ok(std::process::Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?
        .status
        .success())
}

fn resolve_commit(repo: &Utf8Path, revision: &str) -> anyhow::Result<String> {
    let commit = format!("{revision}^{{commit}}");
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", "--end-of-options", &commit])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Git revision {revision:?} does not identify a commit: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[rule("rule_ste_strict_initial_commit_base")]
fn first_parent(repo: &Utf8Path, candidate: &str) -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(["cat-file", "-p", candidate])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "cannot read Git commit {candidate}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let object = String::from_utf8(output.stdout)?;
    let Some(parent) = object.lines().find_map(|line| line.strip_prefix("parent ")) else {
        return Ok(None);
    };
    resolve_commit(repo, parent)
        .map(Some)
        .context("Git HEAD first parent is not available. Fetch more Git history")
}

fn read_commit_family(
    repo: &Utf8Path,
    commit: &str,
    scopes: &[Scope],
) -> anyhow::Result<StatementFamily> {
    let mut family = StatementFamily::default();
    for scope in scopes {
        family
            .requirements
            .extend(read_commit_records::<Requirement>(
                repo,
                commit,
                &scope.id,
                "requirements/req.jsonl",
            )?);
        family.rules.extend(read_commit_records::<Rule>(
            repo,
            commit,
            &scope.id,
            "rules/rule.jsonl",
        )?);
    }
    Ok(family)
}

fn read_working_family(store: &StateStore, scopes: &[Scope]) -> anyhow::Result<StatementFamily> {
    let mut family = StatementFamily::default();
    for scope in scopes {
        family
            .requirements
            .extend(store.list_requirements(&scope.id)?);
        family.rules.extend(store.list_rules(&scope.id)?);
    }
    Ok(family)
}

fn analyze_family_change(
    repo: &Utf8Path,
    base: &StatementFamily,
    candidate: &StatementFamily,
) -> Vec<StatementDiagnostic> {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.to_owned());
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(&layout);
    analyze_changed_statements(
        &base.requirements,
        &base.rules,
        &candidate.requirements,
        &candidate.rules,
        dictionary.as_ref(),
    )
}

fn read_commit_records<T: DeserializeOwned>(
    repo: &Utf8Path,
    commit: &str,
    scope: &ScopeId,
    family_path: &str,
) -> anyhow::Result<Vec<T>> {
    let path = format!(
        ".provenance/state/scopes/{}/{}",
        scope.as_str(),
        family_path
    );
    let listing = std::process::Command::new("git")
        .current_dir(repo)
        .args(["ls-tree", "--name-only", commit, "--", &path])
        .output()?;
    anyhow::ensure!(
        listing.status.success(),
        "cannot inspect Git commit {commit} for {path}: {}",
        String::from_utf8_lossy(&listing.stderr).trim()
    );
    if listing.stdout.is_empty() {
        return Ok(Vec::new());
    }
    let object = format!("{commit}:{path}");
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(["show", &object])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "cannot read {path} from Git commit {commit}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}
