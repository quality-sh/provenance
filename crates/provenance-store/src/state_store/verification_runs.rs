use provenance_core::{
    validate_optional_commit_pin, Capability, RbacClaim, ScopeId, StableId, VerificationMethod,
    VerificationRun, VerificationRunStatus, SUPPORTED_SCHEMA_VERSION,
};
use std::str::FromStr as _;

use super::requirement_reviews::now_millis;
use super::{
    BeginVerificationInput, CompleteVerificationInput, MaterializeVerificationBindingInput,
    MutationAuth, StateStore,
};

impl StateStore {
    /// Begins a verification run (family 14, `execute`). The run shard keeps
    /// its own advisory lock, so this writer goes through the direct-lock
    /// primitive with the same backstop.
    pub fn begin_verification(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: ScopeId,
        input: BeginVerificationInput,
    ) -> anyhow::Result<VerificationRun> {
        let auth = MutationAuth::new(claim, Capability::Execute, &scope_id);
        anyhow::ensure!(
            !input.declared_by.trim().is_empty(),
            "declared_by must not be empty"
        );
        anyhow::ensure!(!input.method.trim().is_empty(), "method must not be empty");
        let method = VerificationMethod::from_str(&input.method)?;
        let file = input.file.ok_or_else(|| {
            anyhow::anyhow!("file is required for a durable verification binding")
        })?;
        let commit = validate_optional_commit_pin(input.commit)?;
        let rules = self.list_rules(&scope_id)?;
        let rule_id = match (input.rule, input.declaration) {
            (Some(rule), None) => {
                let rule_id = StableId::new(rule)?;
                let rule = rules
                    .iter()
                    .find(|rule| rule.id == rule_id)
                    .ok_or_else(|| anyhow::anyhow!("rule `{}` does not exist", rule_id.as_str()))?;
                anyhow::ensure!(!rule.retired, "rule `{}` is retired", rule_id.as_str());
                rule_id
            }
            (None, Some(declaration)) => {
                let rule = rules
                    .iter()
                    .find(|rule| {
                        rule.declared_by.as_deref() == Some(declaration.declared_by.as_str())
                            && rule.declaration_address.as_ref() == Some(&declaration.address)
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "declaration owned by `{}` at `{}` has not been applied",
                            declaration.declared_by,
                            declaration.address.segments().join("/")
                        )
                    })?;
                anyhow::ensure!(
                    !rule.retired,
                    "declaration owned by `{}` at `{}` is retired",
                    declaration.declared_by,
                    declaration.address.segments().join("/")
                );
                rule.id.clone()
            }
            (Some(_), Some(_)) => {
                anyhow::bail!("begin verification accepts either rule or declaration, not both")
            }
            (None, None) => {
                anyhow::bail!("begin verification requires either rule or declaration")
            }
        };
        let binding = self.materialize_verification_binding(
            claim,
            MaterializeVerificationBindingInput {
                scope_id: scope_id.clone(),
                rule_id: rule_id.clone(),
                key: input.key,
                method,
                declared_by: input.declared_by.clone(),
                file: file.clone(),
                symbol: input.symbol.clone(),
            },
        )?;
        let started_at = now_millis()?;
        let path = self.layout.verification_runs_path(&scope_id);
        let lock_path = self.layout.verification_runs_lock_path(&scope_id);
        let run = self.mutate_locked_records(
            &path,
            &lock_path,
            auth.clone(),
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
        self.clear_requirement_reviews(auth, &run.scope_id, &run.rule_id, &run.id, run.started_at)?;
        Ok(run)
    }

    pub fn complete_verification(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        input: CompleteVerificationInput,
    ) -> anyhow::Result<VerificationRun> {
        let run_id = StableId::new(input.run)?;
        let status = VerificationRunStatus::parse_completion(&input.status)?;
        anyhow::ensure!(
            status == VerificationRunStatus::Failed || input.error.is_none(),
            "a passed verification cannot carry an error"
        );
        let completed_at = now_millis()?;
        let path = self.layout.verification_runs_path(scope_id);
        let lock_path = self.layout.verification_runs_lock_path(scope_id);
        self.mutate_locked_records(
            &path,
            &lock_path,
            MutationAuth::new(claim, Capability::Execute, scope_id),
            |records: &mut Vec<VerificationRun>| {
                let run = records
                    .iter_mut()
                    .find(|run| run.scope_id == *scope_id && run.id == run_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("verification run `{}` does not exist", run_id.as_str())
                    })?;
                anyhow::ensure!(
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
