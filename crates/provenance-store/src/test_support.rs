#[path = "../tests/support/initialization.rs"]
mod initialization;
pub use initialization::{allow_reviewer, initialize};

/// The fixture repository at `layout`, opened for `default`, or for the
/// first scope its manifest declares. The open runs publication recovery
/// before it reads the manifest.
pub fn repository(layout: &crate::layout::ProvenanceLayout) -> crate::repository::Repository {
    use crate::repository::{OpenFailure, Repository};
    match Repository::open(layout.root(), "default") {
        Ok(repository) => repository,
        Err(OpenFailure::UnknownScope { declared, .. }) => {
            Repository::open(layout.root(), &declared[0]).unwrap()
        }
        Err(error) => panic!("open the fixture repository {}: {error}", layout.root()),
    }
}

/// A store over the fixture repository at `layout`.
pub fn store(layout: &crate::layout::ProvenanceLayout) -> crate::state_store::StateStore {
    repository(layout).store()
}

/// Runs operation `O` on `host` from its JSON call, `{"context": …, "request": …}`, without
/// lookup by name. A data-free operation has no context. A refusal is framed as the
/// operation declares it.
#[allow(clippy::result_large_err)]
pub async fn call<O, H>(
    call: serde_json::Value,
    host: H,
) -> Result<serde_json::Value, provenance_core::protocol::failure::ErasedFailure>
where
    O: crate::operations::catalog::Operation,
    H: crate::operations::catalog::Invoke,
{
    use crate::operations::catalog::{frame_failure, ContextKind, RequestedContext};
    fn decode<T: serde::de::DeserializeOwned>(value: &serde_json::Value, part: &str) -> T {
        serde_json::from_value(value.clone())
            .unwrap_or_else(|error| panic!("decode the {part}: {error}"))
    }
    let context = &call["context"];
    let selected = match O::CONTEXT {
        ContextKind::Repository => Some(RequestedContext::Repository(decode(context, "context"))),
        ContextKind::Scope => Some(RequestedContext::Scope(decode(context, "context"))),
        ContextKind::Scoped => Some(RequestedContext::Scoped(decode(context, "context"))),
        ContextKind::DataFree => None,
    };
    let request: O::Request = decode(&call["request"], "request");
    let success = host
        .invoke::<O>(selected, request)
        .await
        .map_err(frame_failure::<O>)?;
    Ok(serde_json::to_value(success).expect("encode the success"))
}

/// Accepts the pending submission of one record in `default` as the
/// reviewer that [`allow_reviewer`] names. The manifest must name the
/// reviewer.
pub fn accept_pending(
    store: &crate::state_store::StateStore,
    kind: provenance_core::NodeType,
    id: &str,
) {
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let record = provenance_core::StableId::new(id).unwrap();
    let pending = store
        .record_decision_state(&scope, kind, &record)
        .unwrap()
        .pending
        .unwrap_or_else(|| panic!("{kind:?} {id} has no pending submission"));
    store
        .decide_record_review(
            serde_json::from_value(serde_json::json!({
                "scope_id": "default", "proposal_id": pending.proposal_id,
                "actor": {"identity_type": "human", "id": "reviewer"},
                "decision": "accepted", "rationale": null,
                "canonical_artifact": {"artifact_type": kind.as_str(), "artifact_id": id},
                "feedback": null, "declared_by": null
            }))
            .unwrap(),
        )
        .unwrap();
}

/// Runs `write`, which requests an active or archived status for one record
/// in `default`, and accepts the request as the reviewer. A submission that
/// is pending before the request is accepted first, so the request can open.
pub fn set_status<T>(
    store: &crate::state_store::StateStore,
    kind: provenance_core::NodeType,
    id: &str,
    write: impl FnOnce(&crate::state_store::StateStore) -> anyhow::Result<T>,
) -> T {
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let record = provenance_core::StableId::new(id).unwrap();
    let state = store.record_decision_state(&scope, kind, &record).unwrap();
    if state.pending.is_some() {
        accept_pending(store, kind, id);
    }
    let written = write(store).unwrap();
    let state = store.record_decision_state(&scope, kind, &record).unwrap();
    assert!(
        state.pending.is_some_and(|pending| pending.status_request.is_some()),
        "{kind:?} {id} has no status request"
    );
    accept_pending(store, kind, id);
    written
}

/// Creates a Rule with the status of `input`. An active or archived Rule
/// gets its status through an accepted status request.
pub fn create_rule(
    store: &crate::state_store::StateStore,
    mut input: crate::state_store::CreateRuleInput,
) {
    use provenance_core::{review::RequestedStatus, NodeType, RuleStatus};
    let id = input.id.as_str().to_owned();
    match RequestedStatus::of_rule(&input.status) {
        None => {
            store.create_rule(input).unwrap();
        }
        Some(RequestedStatus::Active) => {
            store.create_rule(input).unwrap();
            accept_pending(store, NodeType::Rule, &id);
        }
        Some(RequestedStatus::Archived) => {
            let archived_in_commit = input.archived_in_commit.take();
            input.status = RuleStatus::Draft;
            let scope_id = input.scope_id.clone();
            store.create_rule(input).unwrap();
            set_status(store, NodeType::Rule, &id, |store| {
                store.update_rule(
                    serde_json::from_value(serde_json::json!({
                        "scope_id": scope_id, "id": id, "status": "archived",
                        "archived_in_commit": archived_in_commit
                    }))
                    .unwrap(),
                )
            });
        }
    }
}
