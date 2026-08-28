//! The rbac merge gate: under an `rbac` section no unchecked family and no
//! unvalidated output survives a merge.
//!
//! Outside the rbac regime `validate_merged_records` keeps its shipped
//! behavior. Here, where the repository carries grants, every merged record
//! is typed against its shard family, the claim must hold `edit` on the
//! scope the shard belongs to, and every disposition row passes the
//! family-12 human-ratification identity check before the merge writes.

use anyhow::Context;
use camino::Utf8Path;
use provenance_core::{
    ensure_disposition_actor_is_human, AssertionRecord, Boundary, Contribution, DispositionRecord,
    Domain, ImplementationBinding, Message, ProposalCard, Question, RbacClaim, RbacSection,
    RequirementReview, Resolution, ScopeId, Source, SynthesisPacket, Thread, Topic,
    VerificationBinding,
};
use serde_json::Value;

use super::validation::{validate_merged_records, ShardFamily};

/// Validates merged output under the rbac regime.
///
/// Refuses an unrecognized shard family, a record that is not of its
/// family's type, a claim that does not hold `edit` on the shard's scope,
/// and a disposition row whose recorded actor has no human-typed assignment.
pub fn validate_rbac_merged_records(
    shard_path: &Utf8Path,
    records: &[Value],
    section: &RbacSection,
    claim: &RbacClaim,
) -> anyhow::Result<()> {
    let family = ShardFamily::for_shard_path(shard_path);
    anyhow::ensure!(
        family != ShardFamily::Unrecognized,
        "rbac: merged output for {shard_path} names no recognized shard family",
    );
    // The shipped checks still apply; this gate only adds to them.
    validate_merged_records(shard_path, records)?;
    validate_typed_family(family, records)?;
    authorize_shard_scope(section, claim, family, shard_path, records)?;
    if matches!(
        family,
        ShardFamily::Dispositions | ShardFamily::IdeationLandings
    ) {
        ensure_disposition_rows_are_human(records, section)?;
    }
    Ok(())
}

fn validate_typed_family(family: ShardFamily, records: &[Value]) -> anyhow::Result<()> {
    macro_rules! typed {
        ($t:ty, $kind:literal) => {{
            for record in records {
                let named = record
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<record with no id>");
                serde_json::from_value::<$t>(record.clone())
                    .with_context(|| format!("merged record {named} is not a {} record", $kind))?;
            }
            Ok(())
        }};
    }
    match family {
        ShardFamily::Sources => typed!(Source, "source"),
        ShardFamily::Domains => typed!(Domain, "domain"),
        ShardFamily::Boundaries => typed!(Boundary, "boundary"),
        ShardFamily::Topics => typed!(Topic, "topic"),
        ShardFamily::Questions => typed!(Question, "question"),
        ShardFamily::Resolutions => typed!(Resolution, "resolution"),
        ShardFamily::Dispositions => typed!(DispositionRecord, "disposition"),
        ShardFamily::Assertions => typed!(AssertionRecord, "assertion"),
        ShardFamily::ProposalCards => typed!(ProposalCard, "proposal"),
        ShardFamily::Contributions => typed!(Contribution, "contribution"),
        ShardFamily::SynthesisPackets => typed!(SynthesisPacket, "synthesis packet"),
        ShardFamily::Threads => typed!(Thread, "thread"),
        ShardFamily::Messages => typed!(Message, "message"),
        ShardFamily::RequirementReviews => typed!(RequirementReview, "requirement review"),
        ShardFamily::ImplementationBindings => {
            typed!(ImplementationBinding, "implementation binding")
        }
        ShardFamily::VerificationBindings => {
            typed!(VerificationBinding, "verification binding")
        }
        // Typed coverage for these four already ran inside
        // `validate_merged_records`.
        ShardFamily::Edges
        | ShardFamily::Requirements
        | ShardFamily::Rules
        | ShardFamily::IdeationLandings
        | ShardFamily::Unrecognized => Ok(()),
    }
}

fn authorize_shard_scope(
    section: &RbacSection,
    claim: &RbacClaim,
    family: ShardFamily,
    shard_path: &Utf8Path,
    records: &[Value],
) -> anyhow::Result<()> {
    use provenance_core::{authorize, Capability, RbacResource};
    if family == ShardFamily::Edges {
        // The edges shard spans scopes; hold `edit` on every scope the
        // merged records name, the fail-safe reading.
        let mut scopes: Vec<ScopeId> = Vec::new();
        for record in records {
            if let Some(name) = record.get("scope_id").and_then(Value::as_str) {
                let scope = ScopeId::new(name)?;
                if !scopes.contains(&scope) {
                    scopes.push(scope);
                }
            }
        }
        anyhow::ensure!(
            !scopes.is_empty(),
            "rbac: merged edge records carry no scope_id to authorize against"
        );
        return authorize(
            Some(claim),
            section,
            Capability::Edit,
            RbacResource::RepoGlobal(&scopes),
        );
    }
    let scope = scoped_family_scope(shard_path)
        .ok_or_else(|| anyhow::anyhow!("rbac: shard path {shard_path} names no scope"))?;
    authorize(
        Some(claim),
        section,
        Capability::Edit,
        RbacResource::Scope(&scope),
    )
}

/// The scope a scoped shard family belongs to: the directory the shard sits
/// in lives under `.provenance/state/scopes/<scope>/`, so the scope is the
/// child of the `scopes` directory on the shard's own path. Walking from the
/// shard toward the root, the directory seen immediately before the `scopes`
/// directory is that child.
fn scoped_family_scope(shard_path: &Utf8Path) -> Option<ScopeId> {
    let mut ancestors = shard_path.parent()?.ancestors();
    let mut child = ancestors.next()?;
    for ancestor in ancestors {
        if ancestor.file_name() == Some("scopes") {
            return ScopeId::new(child.file_name()?).ok();
        }
        child = ancestor;
    }
    None
}

/// Family-12 per row: every merged disposition's recorded actor must resolve
/// to a human-typed assignment.
fn ensure_disposition_rows_are_human(
    records: &[Value],
    section: &RbacSection,
) -> anyhow::Result<()> {
    for (index, record) in records.iter().enumerate() {
        let rows: Vec<Value> = if record.get("proposal_id").is_some() {
            vec![record.clone()]
        } else {
            record
                .get("dispositions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        for row in rows {
            let named = row
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<landing without id>");
            let actor_id = row
                .pointer("/actor/id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "merged record {named} (row {}) carries no disposition actor id",
                        index + 1
                    )
                })?;
            ensure_disposition_actor_is_human(actor_id, &section.assignments)
                .with_context(|| format!("merged record {named}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope_of(path: &str) -> Option<String> {
        scoped_family_scope(Utf8Path::new(path)).map(|scope| scope.as_str().to_owned())
    }

    #[test]
    fn a_scoped_shard_resolves_the_scope_it_sits_under() {
        assert_eq!(
            scope_of(".provenance/state/scopes/default/sources/foo.jsonl").as_deref(),
            Some("default"),
            "the scope directory is the child of the scopes directory on the shard's path"
        );
        assert_eq!(
            scope_of("/repo/.provenance/state/scopes/docs/requirements/req.jsonl").as_deref(),
            Some("docs")
        );
    }

    #[test]
    fn a_shard_outside_any_scope_resolves_no_scope() {
        assert_eq!(scope_of(".provenance/state/edges/edges-00.jsonl"), None);
        assert_eq!(scope_of(".provenance/state/manifest.json"), None);
        assert_eq!(scope_of("sources/foo.jsonl"), None);
    }

    #[test]
    fn a_scope_less_shard_directly_under_scopes_resolves_no_scope() {
        assert_eq!(scope_of(".provenance/state/scopes/foo.jsonl"), None);
    }
}
