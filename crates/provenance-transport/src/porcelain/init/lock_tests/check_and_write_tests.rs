//! Init and a write of the graph: init validates its planned manifest against
//! the graph and writes it in one hold of the publication lock.
use super::{
    change_agents, init_input, init_through_the_port, initialized, lock_is_free, manifest_text,
    options, targets,
};
use crate::porcelain::init::prepare_init;
use camino::Utf8Path;
use provenance_core::{
    DispositionActor, DispositionDecision, IdeationTarget, IdeationTargetType, IdentityType,
    PromotionState, ProposalTraceability, ProposalType, RequirementStatus, ScopeId, StableId,
};
use provenance_store::repository::Repository;
use provenance_store::state_store::{
    CreateDispositionInput, CreateProposalCardInput, CreateRequirementInput,
};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The time that init gives the write to land after its validation.
const WRITE_LIMIT: Duration = Duration::from_secs(5);

type Write = Arc<Mutex<Option<JoinHandle<anyhow::Result<()>>>>>;

/// The Requirement that the proposal of the write targets.
pub(super) fn add_the_target_requirement(repo: &Utf8Path) -> anyhow::Result<()> {
    let store = Repository::open(repo, "default")?.store();
    store.create_requirement(CreateRequirementInput {
        scope_id: ScopeId::new("default")?,
        id: StableId::new("req_overtime")?,
        statement: "The system follows the policy".into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    })?;
    Ok(())
}

/// A write that the first manifest allows: "reviewer" rejects a proposal.
fn reject_a_proposal_as_reviewer(repo: &Utf8Path) -> anyhow::Result<()> {
    let store = Repository::open(repo, "default")?.store();
    let scope = ScopeId::new("default")?;
    store.create_proposal_card(CreateProposalCardInput {
        scope_id: scope.clone(),
        id: StableId::new("proposal_overtime")?,
        proposal_key: "overtime".into(),
        proposal_type: ProposalType::RequirementCandidate,
        title: "Overtime".into(),
        summary: "Clarify the overtime requirement.".into(),
        confidence: None,
        traceability: ProposalTraceability {
            target: IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_overtime")?,
            },
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            supporting_claim_ids: Vec::new(),
        },
        builds_on: Vec::new(),
        promotion_state: PromotionState::Proposed,
        duplicate_of: None,
        superseded_by: None,
        record_revision: None,
        actor: None,
        revises: None,
        revises_rejection: None,
    })?;
    store.create_disposition(CreateDispositionInput {
        scope_id: scope,
        id: StableId::new("disposition_overtime")?,
        proposal_id: StableId::new("proposal_overtime")?,
        decision: DispositionDecision::Rejected,
        rationale: "Reviewed".into(),
        actor: DispositionActor {
            identity_type: IdentityType::Human,
            id: "reviewer".into(),
            name: None,
        },
        canonical_artifact: None,
        external_action: None,
    })?;
    Ok(())
}

/// When init ends its validation, it starts the write on a second thread and
/// waits for the write up to [`WRITE_LIMIT`].
fn write_after_the_validation(repo: &Utf8Path, write: &Write) -> impl Fn() + Send + Sync {
    let repo = repo.to_owned();
    let write = Arc::clone(write);
    move || {
        let repo = repo.clone();
        let write = Arc::clone(&write);
        crate::test_probes::arm("init_validated", move || {
            let writer_repo = repo.clone();
            let handle = std::thread::spawn(move || reject_a_proposal_as_reviewer(&writer_repo));
            let deadline = Instant::now() + WRITE_LIMIT;
            while !handle.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            *write.lock().unwrap() = Some(handle);
        });
    }
}

fn check(repo: &Utf8Path) -> anyhow::Result<()> {
    let repository = Repository::open(repo, "default")?;
    crate::check::validate_repository(&repository.store())
}

/// A second init changes the reviewer list from "reviewer" to
/// "new-reviewer". A write by "reviewer" starts when init has validated its
/// planned manifest. The write waits for init, and the store then refuses it
/// under the new manifest, so the repository check passes.
#[test]
#[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
fn a_write_during_init_does_not_land_between_the_check_and_the_write_of_init() {
    let (_directory, repo, layout) = initialized();
    add_the_target_requirement(&repo).unwrap();
    let write = Write::default();

    init_through_the_port(
        init_input(&repo, &["new-reviewer"]),
        write_after_the_validation(&repo, &write),
    )
    .unwrap();
    let write = write
        .lock()
        .unwrap()
        .take()
        .expect("init did not reach the end of its validation")
        .join()
        .unwrap();

    let manifest = manifest_text(&layout);
    assert!(manifest.contains("new-reviewer"), "{manifest}");
    assert!(
        write.is_err(),
        "the write of \"reviewer\" landed between the check and the write of init"
    );
    let error = format!("{:#}", write.unwrap_err());
    assert!(
        error.contains("does not contain the actor of a review decision"),
        "{error}"
    );
    check(&repo).unwrap();
}

/// A second init plans the reviewer list "new-reviewer". After the plan and
/// before the write, "reviewer" rejects a proposal, which the old manifest
/// allows. Init refuses the planned manifest, because the graph then has a
/// disposition by an actor that the planned list does not allow. The
/// manifest, AGENTS.md, .gitignore, the skill files and the dictionary
/// reference do not change, and the lock is free.
#[test]
#[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
fn a_graph_write_after_the_plan_makes_init_refuse_and_write_nothing() {
    let (_directory, repo, layout) = initialized();
    add_the_target_requirement(&repo).unwrap();
    change_agents(&repo);
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();
    reject_a_proposal_as_reviewer(&repo).unwrap();
    let before = targets(&repo);

    let Err(error) = plan.apply() else {
        panic!(
            "init wrote a planned state that is not valid; the repository check then gives: {}",
            check(&repo).map_or_else(|error| format!("{error:#}"), |()| "no error".to_owned())
        );
    };

    assert_eq!(format!("{error:#}"), REFUSAL);
    assert_eq!(targets(&repo), before, "the refused init changed a file");
    assert!(lock_is_free(&layout), "init kept the publication lock");
}

/// The refusal of init for the disposition by "reviewer".
const REFUSAL: &str = "the planned Provenance state is not valid: The manifest field \
    disposition_actor_ids does not contain the actor of a review decision. To change this field, \
    use the command provenance init --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give \
    --disposition-actor-id one time for each actor that can make a review decision.";
