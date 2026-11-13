use crate::write_error::publication_started;
use camino::Utf8PathBuf;
use provenance_core::{
    validate_optional_commit_pin, ScopeId, StableId, VerificationMethod, VerificationRun,
    VerificationRunStatus,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read};

use super::requirement_reviews::now_millis;
use super::{
    BeginVerificationInput, CompleteVerificationInput, MaterializeVerificationBindingInput,
    StateStore,
};

impl StateStore {
    /// The caller parses the method word; this function receives it typed and
    /// writes it only through `VerificationMethod::to_string()` at the
    /// run-row edge.
    ///
    /// In the section, the checks and the bytes of each write come first.
    /// Then the binding and the cleared reviews that changed publish as one
    /// write set, and the run row goes into the cache last. A start that
    /// changes no binding and clears no review publishes no set.
    pub fn begin_verification(
        &self,
        scope_id: ScopeId,
        input: BeginVerificationInput,
        method: VerificationMethod,
    ) -> anyhow::Result<VerificationRun> {
        self.ensure_writable()?;
        crate::write_error::ensure!(
            InvalidVerificationTarget,
            !input.declared_by.trim().is_empty(),
            "declared_by must not be empty"
        );
        let file = input.file.ok_or_else(|| {
            crate::refusal::refuse!(
                InvalidVerificationTarget,
                "file is required for a durable verification binding"
            )
        })?;
        let commit = validate_optional_commit_pin(input.commit)
            .map_err(|error| crate::refusal::refuse!(InvalidVerificationTarget, "{error:#}"))?;
        self.with_repository_publication(|held| {
            let rule_id = verification_rule(held, &scope_id, input.rule, input.declaration)?;
            let binding =
                held.verification_binding_change(MaterializeVerificationBindingInput {
                    scope_id: scope_id.clone(),
                    rule_id: rule_id.clone(),
                    key: input.key,
                    method,
                    declared_by: input.declared_by.clone(),
                    file: file.clone(),
                    symbol: input.symbol.clone(),
                })?;
            let binding_id = binding.binding.id.clone();
            let run = held.next_run(&binding.binding.scope_id, |id, started_at| {
                VerificationRun {
                    scope_id,
                    id,
                    binding_id: Some(binding_id),
                    rule_id,
                    method: method.to_string(),
                    declared_by: input.declared_by,
                    file: Some(file),
                    symbol: input.symbol,
                    commit,
                    status: VerificationRunStatus::Running,
                    started_at,
                    completed_at: None,
                    error: None,
                }
            })?;
            let shards: Vec<_> = binding
                .shard
                .into_iter()
                .chain(held.cleared_reviews_shard(&run)?)
                .collect();
            let landed = !shards.is_empty();
            held.publish_shards(shards)?;
            // After a set landed, a failure of the run row is a write that
            // began.
            held.add_run_row(&run).map_err(|error| {
                if landed {
                    publication_started(error)
                } else {
                    error
                }
            })?;
            Ok(run)
        })
    }

    /// The run that `run` makes from the start time of the clock and the
    /// next run id that no row of the cache holds and no review names. The
    /// caller holds the section, so no other start takes the id before the
    /// row of the run lands.
    fn next_run(
        &self,
        scope_id: &ScopeId,
        run: impl FnOnce(StableId, i64) -> VerificationRun,
    ) -> anyhow::Result<VerificationRun> {
        let started_at = now_millis(&*self.clock())?;
        let (path, lock_path) = self.run_files(scope_id);
        let rows: Vec<VerificationRun> = crate::jsonl::read_jsonl_locked(&path, &lock_path)?;
        let reviews = self.list_requirement_reviews(scope_id)?;
        let named = reviews.iter().filter_map(|review| review.cleared_by_run.as_ref());
        let taken: BTreeSet<&str> = rows
            .iter()
            .map(|row| &row.id)
            .chain(named)
            .map(StableId::as_str)
            .collect();
        Ok(run(next_run_id(&taken, started_at)?, started_at))
    }

    /// Adds `run` as a new row of the runs cache.
    fn add_run_row(&self, run: &VerificationRun) -> anyhow::Result<()> {
        let (path, lock_path) = self.run_files(&run.scope_id);
        crate::jsonl::mutate_jsonl_locked(
            &path,
            &lock_path,
            |records: &mut Vec<VerificationRun>| {
                records.push(run.clone());
                records.sort_by(|left, right| {
                    left.started_at
                        .cmp(&right.started_at)
                        .then(left.id.as_str().cmp(right.id.as_str()))
                });
                Ok(())
            },
        )
    }

    /// The runs cache of `scope_id` and its lock file.
    fn run_files(&self, scope_id: &ScopeId) -> (Utf8PathBuf, Utf8PathBuf) {
        (
            self.layout.verification_runs_path(scope_id),
            self.layout.verification_runs_lock_path(scope_id),
        )
    }

    pub fn complete_verification(
        &self,
        scope_id: &ScopeId,
        input: CompleteVerificationInput,
    ) -> anyhow::Result<VerificationRun> {
        self.ensure_writable()?;
        let run_id = StableId::new(input.run)
            .map_err(|error| crate::refusal::refuse!(InvalidCompletion, "{error:#}"))?;
        let status = VerificationRunStatus::parse_completion(&input.status)
            .map_err(|error| crate::refusal::refuse!(InvalidCompletion, "{error:#}"))?;
        crate::write_error::ensure!(
            InvalidCompletion,
            status == VerificationRunStatus::Failed || input.error.is_none(),
            "a passed verification cannot carry an error"
        );
        let completed_at = now_millis(&*self.clock())?;
        self.with_repository_publication(|held| {
            let (path, lock_path) = held.run_files(scope_id);
            crate::jsonl::mutate_jsonl_locked(
                &path,
                &lock_path,
                |records: &mut Vec<VerificationRun>| {
                    let run = records
                        .iter_mut()
                        .find(|run| run.scope_id == *scope_id && run.id == run_id)
                        .ok_or_else(|| {
                            crate::refusal::refuse!(
                                InvalidCompletion,
                                "verification run `{}` does not exist",
                                run_id.as_str()
                            )
                        })?;
                    crate::write_error::ensure!(
                        AlreadyComplete,
                        run.status == VerificationRunStatus::Running,
                        "verification run `{}` is already complete",
                        run_id.as_str()
                    );
                    run.status = status;
                    run.completed_at = Some(completed_at.max(run.started_at));
                    run.error = input.error;
                    Ok(run.clone())
                },
            )
        })
    }

    pub fn list_verification_runs(
        &self,
        scope_id: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationRun>> {
        let (path, lock_path) = self.run_files(scope_id);
        crate::jsonl::with_advisory_lock(&lock_path, || {
            if !path.exists() {
                return Ok(Vec::new());
            }
            std::fs::read_to_string(path)?
                .lines()
                .map(|line| serde_json::from_str(line).map_err(Into::into))
                .collect()
        })
    }

    /// Reads verification runs through a bounded typed line reader while the
    /// run file is locked. The digest identifies the exact live input.
    pub(crate) fn read_verification_runs<R>(
        &self,
        scope_id: &ScopeId,
        maximum: usize,
        read: impl FnOnce(
            &str,
            &mut dyn FnMut() -> anyhow::Result<Option<(i64, VerificationRun)>>,
        ) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let (path, lock_path) = self.run_files(scope_id);
        crate::jsonl::with_advisory_lock(&lock_path, || {
            let mut file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let digest = hex::encode(Sha256::digest([]));
                    let mut next = || Ok(None);
                    return read(&digest, &mut next);
                }
                Err(error) => return Err(error.into()),
            };
            let mut hasher = Sha256::new();
            let mut chunk = [0_u8; 8192];
            loop {
                let count = file.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                hasher.update(&chunk[..count]);
            }
            let digest = hex::encode(hasher.finalize());
            let mut reader = BufReader::new(std::fs::File::open(&path)?);
            let mut line_number = 0_i64;
            let mut next = || {
                let mut bytes = Vec::new();
                let count = reader
                    .by_ref()
                    .take(u64::try_from(maximum + 2)?)
                    .read_until(b'\n', &mut bytes)?;
                if count == 0 {
                    return Ok(None);
                }
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                }
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                if bytes.len() > maximum {
                    return Err(
                        provenance_core::protocol::read_failure::ReadFailure::PageRecordTooLarge
                            .into(),
                    );
                }
                line_number += 1;
                let value: serde_json::Value = serde_json::from_slice(&bytes)?;
                crate::state_store::readers::ensure_no_legacy_retirement(
                    &path,
                    usize::try_from(line_number)?,
                    &value,
                )?;
                Ok(Some((line_number, serde_json::from_value(value)?)))
            };
            read(&digest, &mut next)
        })
    }
}

fn verification_rule(
    store: &StateStore,
    scope_id: &ScopeId,
    rule: Option<String>,
    declaration: Option<super::DeclarationReferenceInput>,
) -> anyhow::Result<StableId> {
    let rules = store.list_rules(scope_id)?;
    let rule_id = match (rule, declaration) {
        (Some(rule), None) => {
            let rule_id = StableId::new(rule)
                .map_err(|error| crate::refusal::refuse!(InvalidVerificationTarget, "{error:#}"))?;
            let rule = rules
                .iter()
                .find(|rule| rule.id == rule_id)
                .ok_or_else(|| {
                    crate::refusal::refuse!(
                        InvalidVerificationTarget,
                        "rule `{}` does not exist",
                        rule_id.as_str()
                    )
                })?;
            rule.id.clone()
        }
        (None, Some(declaration)) => {
            let rule = rules
                .iter()
                .find(|rule| {
                    rule.declared_by.as_deref() == Some(declaration.declared_by.as_str())
                        && rule.declaration_address.as_ref() == Some(&declaration.address)
                })
                .ok_or_else(|| {
                    crate::refusal::refuse!(
                        InvalidVerificationTarget,
                        "declaration owned by `{}` at `{}` has not been applied",
                        declaration.declared_by,
                        declaration.address.segments().join("/")
                    )
                })?;
            rule.id.clone()
        }
        (Some(_), Some(_)) => {
            return Err(crate::refusal::refuse!(
                InvalidVerificationTarget,
                "begin verification accepts either rule or declaration, not both"
            ))
        }
        (None, None) => {
            return Err(crate::refusal::refuse!(
                InvalidVerificationTarget,
                "begin verification requires either rule or declaration"
            ))
        }
    };
    Ok(rule_id)
}

fn next_run_id(taken: &BTreeSet<&str>, started_at: i64) -> anyhow::Result<StableId> {
    let base = format!("verification_{started_at}");
    let mut candidate = base.clone();
    let mut suffix = 2_u64;
    while taken.contains(candidate.as_str()) {
        candidate = format!("{base}_{suffix}");
        suffix = suffix
            .checked_add(1)
            .ok_or_else(|| crate::refusal::fault!("verification run id suffix overflow"))?;
    }
    StableId::new(candidate)
}
