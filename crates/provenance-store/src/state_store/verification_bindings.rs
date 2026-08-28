use provenance_core::{
    Capability, RbacClaim, StableId, VerificationBinding, SUPPORTED_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

use super::{MaterializeVerificationBindingInput, MutationAuth, StateStore};
use crate::shards;

impl StateStore {
    pub fn materialize_verification_binding(
        &self,
        claim: Option<&RbacClaim>,
        input: MaterializeVerificationBindingInput,
    ) -> anyhow::Result<VerificationBinding> {
        self.with_repository_publication(|| self.write_verification_binding(claim, input))
    }

    fn write_verification_binding(
        &self,
        claim: Option<&RbacClaim>,
        input: MaterializeVerificationBindingInput,
    ) -> anyhow::Result<VerificationBinding> {
        anyhow::ensure!(!input.key.trim().is_empty(), "key must not be empty");
        anyhow::ensure!(
            !input.declared_by.trim().is_empty(),
            "declared_by must not be empty"
        );
        anyhow::ensure!(!input.file.as_str().is_empty(), "file must not be empty");
        anyhow::ensure!(
            !input.file.as_str().contains('\\')
                && !input.file.is_absolute()
                && !input.file.components().any(|part| {
                    matches!(
                        part,
                        camino::Utf8Component::ParentDir
                            | camino::Utf8Component::RootDir
                            | camino::Utf8Component::Prefix(_)
                    )
                }),
            "file must be a repository-relative path"
        );
        if let Some(symbol) = &input.symbol {
            anyhow::ensure!(!symbol.trim().is_empty(), "symbol must not be empty");
        }
        anyhow::ensure!(
            self.list_rules(&input.scope_id)?
                .iter()
                .any(|rule| rule.id == input.rule_id),
            "rule `{}` does not exist",
            input.rule_id.as_str()
        );

        let id = binding_id(&input)?;
        let binding = VerificationBinding {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: input.scope_id.clone(),
            id: id.clone(),
            rule_id: input.rule_id,
            key: input.key,
            method: input.method,
            declared_by: input.declared_by,
            retired: false,
            file: input.file,
            symbol: input.symbol,
        };
        let path = shards::verification_bindings_path(&self.layout, &input.scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &input.scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<VerificationBinding>| {
            retire_replaced(records, &binding);
            if let Some(existing) = records.iter_mut().find(|record| record.id == id) {
                *existing = binding.clone();
            } else {
                records.push(binding.clone());
            }
            records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
            Ok(binding)
        })
    }
}

/// Retires the relationship an owner-local key replaced in one test file. A
/// run vouches only for the owner, file, and key it just reported, so a key
/// reused by another owner or from another file stays untouched.
fn retire_replaced(records: &mut [VerificationBinding], reported: &VerificationBinding) {
    for record in records.iter_mut().filter(|record| {
        record.declared_by == reported.declared_by
            && record.file == reported.file
            && record.key == reported.key
            && record.rule_id != reported.rule_id
    }) {
        record.retired = true;
    }
}

fn binding_id(input: &MaterializeVerificationBindingInput) -> anyhow::Result<StableId> {
    let identity = format!(
        "{}\0{}\0{}",
        input.declared_by,
        input.rule_id.as_str(),
        input.key
    );
    let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
    StableId::new(format!("verification_binding_{}", &digest[..20]))
}
