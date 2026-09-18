use crate::write_error::{publication_started, SourceFailure, WriteFailure};
use provenance_core::{
    validate_optional_commit_pin, ScopeId, StableId, VerificationMethod, VerificationRun,
    VerificationRunStatus, SUPPORTED_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};
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
    pub fn begin_verification(
        &self,
        scope_id: ScopeId,
        input: BeginVerificationInput,
        method: VerificationMethod,
    ) -> anyhow::Result<VerificationRun> {
        crate::write_error::ensure!(
            InvalidVerificationTarget,
            !input.declared_by.trim().is_empty(),
            "declared_by must not be empty"
        );
        let file = input.file.ok_or_else(|| {
            SourceFailure::wrap(
                WriteFailure::InvalidVerificationTarget,
                anyhow::anyhow!("file is required for a durable verification binding"),
            )
        })?;
        let commit = validate_optional_commit_pin(input.commit)
            .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidVerificationTarget, error))?;
        self.with_repository_publication(|| {
            let rule_id = verification_rule(self, &scope_id, input.rule, input.declaration)?;
            let binding =
                self.materialize_verification_binding(MaterializeVerificationBindingInput {
                    scope_id: scope_id.clone(),
                    rule_id: rule_id.clone(),
                    key: input.key,
                    method,
                    declared_by: input.declared_by.clone(),
                    file: file.clone(),
                    symbol: input.symbol.clone(),
                })?;
            (|| -> anyhow::Result<VerificationRun> {
                crate::test_probes::at("verification_binding_published")?;
                let started_at = now_millis()?;
                let path = self.layout.verification_runs_path(&scope_id);
                let lock_path = self.layout.verification_runs_lock_path(&scope_id);
                let run = crate::jsonl::mutate_jsonl_locked(
                    &path,
                    &lock_path,
                    |records: &mut Vec<VerificationRun>| {
                        let id = next_run_id(records, started_at)?;
                        let run = VerificationRun {
                            schema_version: SUPPORTED_SCHEMA_VERSION,
                            scope_id,
                            id,
                            binding_id: Some(binding.id),
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
                        };
                        records.push(run.clone());
                        records.sort_by(|left, right| {
                            left.started_at
                                .cmp(&right.started_at)
                                .then(left.id.as_str().cmp(right.id.as_str()))
                        });
                        Ok(run)
                    },
                )?;
                self.clear_requirement_reviews(
                    &run.scope_id,
                    &run.rule_id,
                    &run.id,
                    run.started_at,
                )?;
                Ok(run)
            })()
            .map_err(publication_started)
        })
    }

    pub fn complete_verification(
        &self,
        scope_id: &ScopeId,
        input: CompleteVerificationInput,
    ) -> anyhow::Result<VerificationRun> {
        let run_id = StableId::new(input.run)
            .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidCompletion, error))?;
        let status = VerificationRunStatus::parse_completion(&input.status)
            .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidCompletion, error))?;
        crate::write_error::ensure!(
            InvalidCompletion,
            status == VerificationRunStatus::Failed || input.error.is_none(),
            "a passed verification cannot carry an error"
        );
        let completed_at = now_millis()?;
        self.with_repository_publication(|| {
            let path = self.layout.verification_runs_path(scope_id);
            let lock_path = self.layout.verification_runs_lock_path(scope_id);
            crate::jsonl::mutate_jsonl_locked(
                &path,
                &lock_path,
                |records: &mut Vec<VerificationRun>| {
                    let run = records
                        .iter_mut()
                        .find(|run| run.scope_id == *scope_id && run.id == run_id)
                        .ok_or_else(|| {
                            SourceFailure::wrap(
                                WriteFailure::InvalidCompletion,
                                anyhow::anyhow!(
                                    "verification run `{}` does not exist",
                                    run_id.as_str()
                                ),
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
                    let digest = format!("{:x}", Sha256::digest([]));
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
            let digest = format!("{:x}", hasher.finalize());
            let mut reader = BufReader::new(std::fs::File::open(&path)?);
            let mut line_number = 0_i64;
            let mut next = || {
                let maximum = crate::cache::read::page::RESOURCE_RECORD_BYTES;
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
                crate::state_store::readers::ensure_supported_record_version(
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
            let rule_id = StableId::new(rule).map_err(|error| {
                SourceFailure::wrap(WriteFailure::InvalidVerificationTarget, error)
            })?;
            let rule = rules
                .iter()
                .find(|rule| rule.id == rule_id)
                .ok_or_else(|| {
                    SourceFailure::wrap(
                        WriteFailure::InvalidVerificationTarget,
                        anyhow::anyhow!("rule `{}` does not exist", rule_id.as_str()),
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
                    SourceFailure::wrap(
                        WriteFailure::InvalidVerificationTarget,
                        anyhow::anyhow!(
                            "declaration owned by `{}` at `{}` has not been applied",
                            declaration.declared_by,
                            declaration.address.segments().join("/")
                        ),
                    )
                })?;
            rule.id.clone()
        }
        (Some(_), Some(_)) => {
            return Err(SourceFailure::wrap(
                WriteFailure::InvalidVerificationTarget,
                anyhow::anyhow!("begin verification accepts either rule or declaration, not both"),
            ))
        }
        (None, None) => {
            return Err(SourceFailure::wrap(
                WriteFailure::InvalidVerificationTarget,
                anyhow::anyhow!("begin verification requires either rule or declaration"),
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
            .ok_or_else(|| anyhow::anyhow!("verification run id suffix overflow"))?;
    }
    StableId::new(candidate)
}
