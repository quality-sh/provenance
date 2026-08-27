use camino::Utf8Path;
use provenance_core::{Requirement, Rule, ScopeId};
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

#[rule("rule_ste_manual_changed_statement_report")]
pub(super) fn changed_statements_from_head(
    store: &StateStore,
    repo: &Utf8Path,
) -> anyhow::Result<Vec<StatementDiagnostic>> {
    if !has_head(repo)? {
        return Ok(Vec::new());
    }

    let manifest = store.manifest()?;
    let mut base_requirements = Vec::new();
    let mut base_rules = Vec::new();
    let mut candidate_requirements = Vec::new();
    let mut candidate_rules = Vec::new();
    for scope in manifest.scopes {
        base_requirements.extend(read_commit_records::<Requirement>(
            repo,
            "HEAD",
            &scope.id,
            "requirements/req.jsonl",
        )?);
        base_rules.extend(read_commit_records::<Rule>(
            repo,
            "HEAD",
            &scope.id,
            "rules/rule.jsonl",
        )?);
        candidate_requirements.extend(store.list_requirements(&scope.id)?);
        candidate_rules.extend(store.list_rules(&scope.id)?);
    }
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.to_owned());
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(&layout);
    Ok(analyze_changed_statements(
        &base_requirements,
        &base_rules,
        &candidate_requirements,
        &candidate_rules,
        dictionary.as_ref(),
    ))
}

#[rule("rule_ste_strict_committed_statement_selection")]
pub(super) fn changed_statements_from_commits(
    store: &StateStore,
    repo: &Utf8Path,
    explicit_base: Option<&str>,
) -> anyhow::Result<CommittedStatementAnalysis> {
    let candidate_commit = resolve_commit(repo, "HEAD")?;
    let base_commit = match explicit_base {
        Some(base) => Some(resolve_commit(repo, base)?),
        None => first_parent(repo, &candidate_commit)?,
    };
    let manifest = store.manifest()?;
    let mut base_requirements = Vec::new();
    let mut base_rules = Vec::new();
    let mut candidate_requirements = Vec::new();
    let mut candidate_rules = Vec::new();
    for scope in manifest.scopes {
        if let Some(base) = &base_commit {
            base_requirements.extend(read_commit_records::<Requirement>(
                repo,
                base,
                &scope.id,
                "requirements/req.jsonl",
            )?);
            base_rules.extend(read_commit_records::<Rule>(
                repo,
                base,
                &scope.id,
                "rules/rule.jsonl",
            )?);
        }
        candidate_requirements.extend(read_commit_records::<Requirement>(
            repo,
            &candidate_commit,
            &scope.id,
            "requirements/req.jsonl",
        )?);
        candidate_rules.extend(read_commit_records::<Rule>(
            repo,
            &candidate_commit,
            &scope.id,
            "rules/rule.jsonl",
        )?);
    }
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.to_owned());
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(&layout);
    let diagnostics = analyze_changed_statements(
        &base_requirements,
        &base_rules,
        &candidate_requirements,
        &candidate_rules,
        dictionary.as_ref(),
    );
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
    resolve_commit(repo, parent).map(Some).map_err(|_| {
        anyhow::anyhow!("Git HEAD first parent is not available. Fetch more Git history")
    })
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
