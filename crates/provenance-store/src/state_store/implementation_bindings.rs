use std::collections::BTreeSet;

use provenance_core::{
    Capability, ImplementationBinding, RbacClaim, Rule, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

use super::typed_specs::DesiredTypedGraph;
use super::{MaterializeImplementationBindingInput, MutationAuth, StateStore, TypedFieldChange};
use crate::shards;

pub(super) struct Reconciliation {
    pub records: Vec<ImplementationBinding>,
    pub active: Vec<ImplementationBinding>,
    pub changes: Vec<(StableId, TypedFieldChange)>,
}

pub(super) fn reconcile(
    store: &StateStore,
    scope_id: &ScopeId,
    graph: DesiredTypedGraph<'_>,
    canonical_rules: &[Rule],
    preserve_omitted_for: &BTreeSet<String>,
) -> anyhow::Result<Reconciliation> {
    let mut records = store.list_implementation_bindings(scope_id)?;
    let mut desired = desired_bindings(scope_id, graph, &records)?;
    for binding in records.iter().filter(|binding| {
        !binding.retired && preserve_omitted_for.contains(binding.rule_id.as_str())
    }) {
        if let Some(generated) = desired
            .iter_mut()
            .find(|desired| desired.rule_id == binding.rule_id)
        {
            *generated = binding.clone();
        } else {
            desired.push(binding.clone());
        }
    }
    desired.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let mut changes = Vec::new();

    for binding in &desired {
        if let Some(record) = records
            .iter_mut()
            .find(|record| record.rule_id == binding.rule_id)
        {
            let before = target(record);
            *record = binding.clone();
            record_change(&mut changes, record.rule_id.clone(), before, target(record));
        } else {
            records.push(binding.clone());
            record_change(
                &mut changes,
                binding.rule_id.clone(),
                serde_json::Value::Null,
                target(binding),
            );
        }
    }

    let desired_rule_ids = desired
        .iter()
        .map(|binding| &binding.rule_id)
        .collect::<Vec<_>>();
    for record in records.iter_mut().filter(|binding| {
        !binding.retired
            && binding.declared_by == graph.owner
            && !desired_rule_ids.contains(&&binding.rule_id)
            && rule_belongs_to_spec(canonical_rules, &binding.rule_id, graph.owner, graph.spec)
    }) {
        let before = target(record);
        record.retired = true;
        record_change(
            &mut changes,
            record.rule_id.clone(),
            before,
            serde_json::Value::Null,
        );
    }

    records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok(Reconciliation {
        records,
        active: desired,
        changes,
    })
}

fn desired_bindings(
    scope_id: &ScopeId,
    graph: DesiredTypedGraph<'_>,
    existing: &[ImplementationBinding],
) -> anyhow::Result<Vec<ImplementationBinding>> {
    graph
        .rules
        .iter()
        .filter_map(|rule| {
            rule.implementation
                .as_ref()
                .map(|implementation| (rule, implementation))
        })
        .map(|(rule, implementation)| {
            let address = super::typed_specs::rule_address(graph.spec, rule)?;
            let rule_id = graph.rule_ids[&address].clone();
            validate_target(&implementation.file, &implementation.symbol)?;
            if let Some(record) = existing.iter().find(|record| record.rule_id == rule_id) {
                anyhow::ensure!(
                    record.declared_by == graph.owner,
                    "rule `{}` implementation is owned by `{}`",
                    rule_id.as_str(),
                    record.declared_by
                );
            }
            Ok(ImplementationBinding {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id: binding_id(&rule_id)?,
                rule_id,
                declared_by: graph.owner.to_string(),
                retired: false,
                file: implementation.file.clone(),
                symbol: implementation.symbol.clone(),
            })
        })
        .collect()
}

impl StateStore {
    pub fn materialize_implementation_binding(
        &self,
        claim: Option<&RbacClaim>,
        input: MaterializeImplementationBindingInput,
    ) -> anyhow::Result<ImplementationBinding> {
        self.with_repository_publication(|| {
            self.ensure_mutation_authorized(MutationAuth::new(
                claim,
                Capability::Execute,
                &input.scope_id,
            ))?;
            anyhow::ensure!(
                !input.declared_by.trim().is_empty(),
                "declared_by must not be empty"
            );
            validate_target(&input.file, &input.symbol)?;
            anyhow::ensure!(
                self.layout.root().join(&input.file).is_file(),
                "implementation file `{}` does not exist",
                input.file
            );
            anyhow::ensure!(
                self.list_rules(&input.scope_id)?
                    .iter()
                    .any(|rule| rule.id == input.rule_id),
                "rule `{}` does not exist",
                input.rule_id.as_str()
            );
            let id = binding_id(&input.rule_id)?;
            let binding = ImplementationBinding {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: input.scope_id.clone(),
                id,
                rule_id: input.rule_id,
                declared_by: input.declared_by,
                retired: false,
                file: input.file,
                symbol: input.symbol,
            };
            let path = shards::implementation_bindings_path(&self.layout, &input.scope_id);
            let auth = MutationAuth::new(claim, Capability::Execute, &input.scope_id);
            self.mutate_jsonl_records(&path, auth, |records: &mut Vec<ImplementationBinding>| {
                anyhow::ensure!(
                    !records.iter().any(|record| {
                        record.rule_id == binding.rule_id
                            && record.declared_by != binding.declared_by
                    }),
                    "rule `{}` implementation is owned by another declaration owner",
                    binding.rule_id.as_str()
                );
                if let Some(record) = records
                    .iter_mut()
                    .find(|record| record.rule_id == binding.rule_id)
                {
                    *record = binding.clone();
                } else {
                    records.push(binding.clone());
                }
                records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
                Ok(binding)
            })
        })
    }
}

fn rule_belongs_to_spec(rules: &[Rule], rule_id: &StableId, owner: &str, spec: &str) -> bool {
    rules.iter().any(|rule| {
        rule.id == *rule_id
            && rule.declared_by.as_deref() == Some(owner)
            && rule
                .declaration_address
                .as_ref()
                .is_some_and(|address| address.segments().first().is_some_and(|part| part == spec))
    })
}

fn target(binding: &ImplementationBinding) -> serde_json::Value {
    if binding.retired {
        serde_json::Value::Null
    } else {
        serde_json::json!({
            "file": binding.file,
            "symbol": binding.symbol,
        })
    }
}

fn record_change(
    changes: &mut Vec<(StableId, TypedFieldChange)>,
    rule_id: StableId,
    before: serde_json::Value,
    after: serde_json::Value,
) {
    if before != after {
        changes.push((
            rule_id,
            TypedFieldChange {
                field: "implementation".to_string(),
                before,
                after,
            },
        ));
    }
}

fn validate_target(file: &camino::Utf8Path, symbol: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !file.as_str().is_empty(),
        "implementation file must not be empty"
    );
    anyhow::ensure!(
        !file.as_str().contains('\\')
            && !file.is_absolute()
            && !file.components().any(|part| {
                matches!(
                    part,
                    camino::Utf8Component::ParentDir
                        | camino::Utf8Component::RootDir
                        | camino::Utf8Component::Prefix(_)
                )
            }),
        "implementation file must be a repository-relative path"
    );
    anyhow::ensure!(
        !symbol.trim().is_empty(),
        "implementation symbol must not be empty"
    );
    Ok(())
}

fn binding_id(rule_id: &StableId) -> anyhow::Result<StableId> {
    let digest = format!("{:x}", Sha256::digest(rule_id.as_str().as_bytes()));
    StableId::new(format!("implementation_binding_{}", &digest[..20]))
}
