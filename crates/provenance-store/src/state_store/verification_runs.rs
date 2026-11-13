use crate::storage::{Atomicity, WriteSet};
use crate::write_error::{publication_started, PublicationStarted};
use provenance_core::{
    validate_optional_commit_pin, ScopeId, StableId, VerificationMethod, VerificationRun,
    VerificationRunStatus,
};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read};

use super::requirement_reviews::now_millis;
use super::verification_bindings::BindingChange;
use super::{
    BeginVerificationInput, CompleteVerificationInput, MaterializeVerificationBindingInput,
    StateStore,
};

impl StateStore {
    /// The caller parses the method word; this function receives it typed and
    /// writes it only through `VerificationMethod::to_string()` at the
    /// run-row edge.
    ///
    /// The checks come first. Then the run row goes into the cache, and the
    /// binding and the cleared reviews publish as one per-key write set. When
    /// the storage refuses the set, the run row is removed again. When the
    /// outcome of the set is not known, the run row stays.
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
            let change = MaterializeVerificationBindingInput {
                scope_id: scope_id.clone(),
                rule_id: rule_id.clone(),
                key: input.key,
                method,
                declared_by: input.declared_by.clone(),
                file: file.clone(),
                symbol: input.symbol.clone(),
            };
            let binding = held.verification_binding_change(change)?;
            let binding_id = binding.binding.id.clone();
            let scope = &binding.binding.scope_id;
            let run = held.add_run_row(scope, |id, started_at| VerificationRun {
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
            });
            // A failure of the run row keeps the answer `write_failed`.
            let run = run.map_err(publication_started)?;
            if let Err(error) = held.publish_begin_set(binding, &run) {
                return Err(held.withdraw_refused_run(&run, error));
            }
            Ok(run)
        })
    }

    /// Adds the run that `run` makes from the next free run id and the start
    /// time of the clock as a new row of the cache.
    fn add_run_row(
        &self,
        scope_id: &ScopeId,
        run: impl FnOnce(StableId, i64) -> VerificationRun,
    ) -> anyhow::Result<VerificationRun> {
        let started_at = now_millis(&*self.clock())?;
        let path = self.layout.verification_runs_path(scope_id);
        let lock_path = self.layout.verification_runs_lock_path(scope_id);
        crate::jsonl::mutate_jsonl_locked(
            &path,
            &lock_path,
            |records: &mut Vec<VerificationRun>| {
                let run = run(next_run_id(records, started_at)?, started_at);
                records.push(run.clone());
                records.sort_by(|left, right| {
                    left.started_at
                        .cmp(&right.started_at)
                        .then(left.id.as_str().cmp(right.id.as_str()))
                });
                Ok(run)
            },
        )
    }

    /// Publishes the binding and the reviews that `run` clears as one
    /// per-key write set.
    fn publish_begin_set(
        &self,
        binding: BindingChange,
        run: &VerificationRun,
    ) -> anyhow::Result<()> {
        let mut writes =
            WriteSet::new(Atomicity::PerKey).put(self.state_key(&binding.path)?, binding.bytes);
        if let Some((path, bytes)) = self.cleared_requirement_reviews(run)? {
            writes = writes.put(self.state_key(&path)?, bytes);
        }
        self.publish_set(writes)
    }

    /// Removes the row of `run` when no key of its write set landed, and
    /// gives `error` as it is. When landing began, the row stays and the
    /// answer is a write failure. When the removal fails, the row stays and
    /// the answer is `error`.
    fn withdraw_refused_run(&self, run: &VerificationRun, error: anyhow::Error) -> anyhow::Error {
        if error.downcast_ref::<PublicationStarted>().is_some() {
            return error;
        }
        let path = self.layout.verification_runs_path(&run.scope_id);
        let lock_path = self.layout.verification_runs_lock_path(&run.scope_id);
        let _removed = crate::jsonl::mutate_jsonl_locked(
            &path,
            &lock_path,
            |records: &mut Vec<VerificationRun>| {
                records.retain(|record| record.id != run.id);
                Ok(())
            },
        );
        error
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
            let path = held.layout.verification_runs_path(scope_id);
            let lock_path = held.layout.verification_runs_lock_path(scope_id);
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
        let path = self.layout.verification_runs_path(scope_id);
        let lock_path = self.layout.verification_runs_lock_path(scope_id);
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
        let path = self.layout.verification_runs_path(scope_id);
        let lock_path = self.layout.verification_runs_lock_path(scope_id);
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

fn next_run_id(records: &[VerificationRun], started_at: i64) -> anyhow::Result<StableId> {
    let base = format!("verification_{started_at}");
    let mut candidate = base.clone();
    let mut suffix = 2_u64;
    while records.iter().any(|run| run.id.as_str() == candidate) {
        candidate = format!("{base}_{suffix}");
        suffix = suffix
            .checked_add(1)
            .ok_or_else(|| crate::refusal::fault!("verification run id suffix overflow"))?;
    }
    StableId::new(candidate)
}
