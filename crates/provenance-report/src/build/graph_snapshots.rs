//! Reading graph record families at a commit and diffing two snapshots.
//!
//! The narrow adapter between the producer and Git: one `git ls-tree` probe
//! and one `git show` read per shard file, mirroring how the statement
//! report reads committed families. All collection order comes from sorted
//! sets, so shard line order never reaches the diff.

use crate::envelope::{ChangeKind, CommitRole, GraphChange, RecordKind, RelationChange};
use anyhow::Context;
use camino::Utf8Path;
use provenance_core::{Requirement, Resolution, Rule, ScopeId, Source};
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

#[cfg(test)]
mod tests;

/// One commit's graph record families for one scope.
#[derive(Debug, Default)]
pub(super) struct GraphSnapshot {
    pub requirements: Vec<Requirement>,
    pub rules: Vec<Rule>,
    pub resolutions: Vec<Resolution>,
    pub sources: Vec<Source>,
}

/// What a commit's graph read produced.
#[derive(Debug)]
pub(super) enum SnapshotRead {
    /// At least one shard file existed and every present file parsed.
    Present(GraphSnapshot),
    /// No shard file existed at the commit.
    Absent,
    /// A shard file existed but its records did not parse.
    Incompatible(String),
}

/// The sorted relation key used for diffs: relation name, target kind, id.
type RelationKey = (String, RecordKind, String);

/// Read one shard file at a commit. `Ok(None)` says the file is absent, the
/// normal state for a scope that did not exist yet.
fn read_shard<T: serde::de::DeserializeOwned>(
    repo: &Utf8Path,
    commit: &str,
    scope: &ScopeId,
    shard: &str,
    role: CommitRole,
) -> anyhow::Result<Option<Vec<T>>> {
    let path = format!(".provenance/state/scopes/{}/{}", scope.as_str(), shard);
    let listing = Command::new("git")
        .current_dir(repo)
        .args(["ls-tree", "--name-only", commit, "--", &path])
        .output()?;
    anyhow::ensure!(
        listing.status.success(),
        "cannot inspect Git commit {commit} for {path}: {}",
        String::from_utf8_lossy(&listing.stderr).trim()
    );
    if listing.stdout.is_empty() {
        return Ok(None);
    }
    let object = format!("{commit}:{path}");
    let output = Command::new("git")
        .current_dir(repo)
        .args(["show", &object])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "cannot read {path} from Git commit {commit}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let text = String::from_utf8(output.stdout)?;
    let mut records = Vec::new();
    for (index, line) in text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
    {
        let line_number = index + 1;
        let context =
            || format!("record in {path} line {line_number} at commit {commit} does not parse");
        let value: serde_json::Value = serde_json::from_str(line).with_context(context)?;
        // Mirror the store reader's admission check. Historical bases stay readable.
        if role == CommitRole::Head && value.get("retired").is_some() {
            let name = value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| "record".to_string(), |id| format!("record {id}"));
            anyhow::bail!(
                "{path} line {line_number} at commit {commit}: {name} contains legacy field `retired`; \
                 canonical JSONL requires record-deletion migration 026 before this build can read it"
            );
        }
        records.push(serde_json::from_value::<T>(value).with_context(context)?);
    }
    Ok(Some(records))
}

/// Read the typed evidence bindings at the head commit. A missing shard file is
/// the normal state for a store that never recorded bindings.
pub(super) fn read_bindings(
    repo: &Utf8Path,
    commit: &str,
    scope: &ScopeId,
) -> anyhow::Result<(
    Vec<provenance_core::VerificationBinding>,
    Vec<provenance_core::ImplementationBinding>,
)> {
    let verifications = read_shard::<provenance_core::VerificationBinding>(
        repo,
        commit,
        scope,
        "verifications/binding.jsonl",
        CommitRole::Head,
    )?
    .unwrap_or_default();
    let implementations = read_shard::<provenance_core::ImplementationBinding>(
        repo,
        commit,
        scope,
        "implementations/binding.jsonl",
        CommitRole::Head,
    )?
    .unwrap_or_default();
    Ok((verifications, implementations))
}

/// Read the four report-relevant families at a commit. A read can always
/// produce a verdict: absence and incompatibility are verdicts, not errors.
pub(super) fn read_snapshot(
    repo: &Utf8Path,
    commit: &str,
    scope: &ScopeId,
    role: CommitRole,
) -> SnapshotRead {
    let requirements =
        read_shard::<Requirement>(repo, commit, scope, "requirements/req.jsonl", role);
    let rules = read_shard::<Rule>(repo, commit, scope, "rules/rule.jsonl", role);
    let resolutions = read_shard::<Resolution>(repo, commit, scope, "resolutions/res.jsonl", role);
    let sources = read_shard::<Source>(repo, commit, scope, "sources/source.jsonl", role);
    let all_absent = matches!(requirements, Ok(None))
        && matches!(rules, Ok(None))
        && matches!(resolutions, Ok(None))
        && matches!(sources, Ok(None));
    if all_absent {
        // No shard file existed at this commit: the store did not exist yet.
        return SnapshotRead::Absent;
    }
    let mut failure: Option<String> = None;
    let snapshot = GraphSnapshot {
        requirements: unwrap_family(requirements, "requirements", &mut failure),
        rules: unwrap_family(rules, "rules", &mut failure),
        resolutions: unwrap_family(resolutions, "resolutions", &mut failure),
        sources: unwrap_family(sources, "sources", &mut failure),
    };
    if let Some(reason) = failure {
        return SnapshotRead::Incompatible(reason);
    }
    SnapshotRead::Present(snapshot)
}

/// Unwrap one family read, recording the first failure as an
/// incompatibility reason while keeping every readable family.
fn unwrap_family<T>(
    records: anyhow::Result<Option<Vec<T>>>,
    family: &str,
    failure: &mut Option<String>,
) -> Vec<T> {
    match records {
        Ok(read) => read.unwrap_or_default(),
        Err(error) => {
            failure.get_or_insert_with(|| format!("{family}: {error:#}"));
            Vec::new()
        }
    }
}

/// Diff two snapshots into report graph changes, sorted by the renderer's
/// canonical order so shard order and read order never matter.
pub(super) fn diff_snapshots(base: &GraphSnapshot, head: &GraphSnapshot) -> Vec<GraphChange> {
    let mut changes = Vec::new();
    diff_records(
        &base.requirements,
        &head.requirements,
        RecordKind::Requirement,
        |record| record.id.as_str(),
        |record| record.statement.clone(),
        requirement_lifecycle,
        requirement_relations,
        &mut changes,
    );
    diff_records(
        &base.rules,
        &head.rules,
        RecordKind::Rule,
        |record| record.id.as_str(),
        |record| record.statement.clone(),
        rule_lifecycle,
        rule_relations,
        &mut changes,
    );
    diff_records(
        &base.resolutions,
        &head.resolutions,
        RecordKind::Resolution,
        |record| record.id.as_str(),
        |record| record.title.clone(),
        resolution_lifecycle,
        resolution_relations,
        &mut changes,
    );
    diff_records(
        &base.sources,
        &head.sources,
        RecordKind::Source,
        |record| record.id.as_str(),
        |record| record.name.clone(),
        |_: &Source| "current".to_string(),
        source_relations,
        &mut changes,
    );
    // The renderer's canonical normalization sorts every emitted collection,
    // so no local order leaks into the envelope bytes.
    changes
}

/// Diff one record family by id. Only changed payloads become `changed`
/// rows; a record whose statement, lifecycle and relations all match is
/// carried over silently.
#[allow(clippy::too_many_arguments)]
fn diff_records<'a, T>(
    base: &'a [T],
    head: &'a [T],
    kind: RecordKind,
    id: impl Fn(&'a T) -> &'a str,
    statement: impl Fn(&'a T) -> String,
    lifecycle: impl Fn(&'a T) -> String,
    relations: impl Fn(&'a T) -> BTreeSet<RelationKey>,
    changes: &mut Vec<GraphChange>,
) {
    let base_by_id: BTreeMap<&str, &T> = base.iter().map(|record| (id(record), record)).collect();
    let head_ids: BTreeSet<&str> = head.iter().map(&id).collect();
    for record in head {
        let record_id = id(record);
        let after_statement = statement(record);
        let after_lifecycle = lifecycle(record);
        let after_relations = relations(record);
        let Some(before) = base_by_id.get(record_id) else {
            changes.push(change_with(
                kind,
                ChangeKind::Added,
                record_id,
                Some(after_statement),
                None,
                None,
                Some(after_lifecycle),
                after_relations,
                BTreeSet::new(),
            ));
            continue;
        };
        let before_lifecycle = lifecycle(before);
        let before_relations = relations(before);
        if statement(before) == after_statement
            && before_lifecycle == after_lifecycle
            && before_relations == after_relations
        {
            continue;
        }
        changes.push(change_with(
            kind,
            ChangeKind::Changed,
            record_id,
            Some(after_statement),
            Some(statement(before)),
            Some(before_lifecycle),
            Some(after_lifecycle),
            after_relations
                .difference(&before_relations)
                .cloned()
                .collect(),
            before_relations
                .difference(&after_relations)
                .cloned()
                .collect(),
        ));
    }
    for record in base {
        if !head_ids.contains(id(record)) {
            changes.push(change_with(
                kind,
                ChangeKind::Removed,
                id(record),
                Some(statement(record)),
                None,
                Some(lifecycle(record)),
                None,
                BTreeSet::new(),
                BTreeSet::new(),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn change_with(
    kind: RecordKind,
    change: ChangeKind,
    id: &str,
    statement: Option<String>,
    statement_before: Option<String>,
    lifecycle_before: Option<String>,
    lifecycle_after: Option<String>,
    relations_added: BTreeSet<RelationKey>,
    relations_removed: BTreeSet<RelationKey>,
) -> GraphChange {
    let to_changes = |set: BTreeSet<RelationKey>| {
        set.into_iter()
            .map(|(relation, target_kind, target_id)| RelationChange {
                relation,
                target_kind,
                target_id,
            })
            .collect::<Vec<_>>()
    };
    GraphChange {
        kind,
        change,
        id: id.to_string(),
        statement,
        statement_before,
        lifecycle_before,
        lifecycle_after,
        relations_added: to_changes(relations_added),
        relations_removed: to_changes(relations_removed),
    }
}

fn relation(name: &str, kind: RecordKind, id: &str) -> RelationKey {
    (name.to_string(), kind, id.to_string())
}

fn requirement_lifecycle(record: &Requirement) -> String {
    let status = match record.status {
        provenance_core::RequirementStatus::Active => "active",
        provenance_core::RequirementStatus::Discovery => "discovery",
        provenance_core::RequirementStatus::Refinement => "refinement",
        provenance_core::RequirementStatus::Resolved => "resolved",
    };
    status.to_string()
}

fn rule_lifecycle(record: &Rule) -> String {
    let status = match record.status {
        provenance_core::RuleStatus::Draft => "draft",
        provenance_core::RuleStatus::Review => "review",
        provenance_core::RuleStatus::Active => "active",
        provenance_core::RuleStatus::Deprecated => "deprecated",
        provenance_core::RuleStatus::Archived => "archived",
    };
    status.to_string()
}

fn resolution_lifecycle(record: &Resolution) -> String {
    let status = match record.status {
        provenance_core::ResolutionStatus::Draft => "draft",
        provenance_core::ResolutionStatus::Review => "review",
        provenance_core::ResolutionStatus::Proposed => "proposed",
        provenance_core::ResolutionStatus::Approved => "approved",
        provenance_core::ResolutionStatus::Rejected => "rejected",
        provenance_core::ResolutionStatus::Revised => "revised",
        provenance_core::ResolutionStatus::Superseded => "superseded",
        provenance_core::ResolutionStatus::Abandoned => "abandoned",
    };
    status.to_string()
}

fn requirement_relations(record: &Requirement) -> BTreeSet<RelationKey> {
    let mut relations = BTreeSet::new();
    if let Some(target) = &record.refines {
        relations.insert(relation(
            "refines",
            RecordKind::Requirement,
            target.as_str(),
        ));
    }
    for reference in &record.source_refs {
        relations.insert(relation(
            "cites",
            RecordKind::Source,
            reference.source_id.as_str(),
        ));
    }
    if let Some(target) = &record.spawned_by {
        relations.insert(relation(
            "spawned_by",
            RecordKind::Resolution,
            target.as_str(),
        ));
    }
    for target in &record.depends_on {
        relations.insert(relation(
            "depends_on",
            RecordKind::Requirement,
            target.as_str(),
        ));
    }
    for target in &record.supersedes {
        relations.insert(relation(
            "supersedes",
            RecordKind::Requirement,
            target.as_str(),
        ));
    }
    relations
}

fn rule_relations(record: &Rule) -> BTreeSet<RelationKey> {
    let mut relations = BTreeSet::new();
    for target in &record.requirement_ids {
        relations.insert(relation("serves", RecordKind::Requirement, target.as_str()));
    }
    for target in &record.resolution_ids {
        relations.insert(relation(
            "produced_by",
            RecordKind::Resolution,
            target.as_str(),
        ));
    }
    relations
}

fn resolution_relations(record: &Resolution) -> BTreeSet<RelationKey> {
    let mut relations = BTreeSet::new();
    for target in &record.requirement_ids {
        relations.insert(relation(
            "resolves",
            RecordKind::Requirement,
            target.as_str(),
        ));
    }
    for target in &record.supersedes {
        relations.insert(relation(
            "supersedes",
            RecordKind::Resolution,
            target.as_str(),
        ));
    }
    relations
}

fn source_relations(record: &Source) -> BTreeSet<RelationKey> {
    record
        .supersedes
        .iter()
        .map(|target| relation("supersedes", RecordKind::Source, target.as_str()))
        .collect()
}
