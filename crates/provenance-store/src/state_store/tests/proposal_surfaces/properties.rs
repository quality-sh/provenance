//! Property verification for the proposal surfacing decision.
//!
//! Inputs come from a hand-rolled deterministic generator so the run is
//! reproducible without a proptest dependency. The expectations below are
//! restated over sets, independently of the loops the decision itself runs.

use crate::state_store::proposal_surfaces::surfacing_reasons;
use crate::state_store::{ProposalDemand, ProposalSurfaceReason};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    IdeationEvidenceReference, IdeationEvidenceType, IdeationTarget, IdeationTargetType,
    PromotionState, ProposalCard, ProposalTraceability, ProposalType, ScopeId, StableId,
};
use provenance_macros::verifies;
use std::collections::BTreeSet;

const CASES: u64 = 512;
const FILES: [&str; 4] = [
    "src/payroll.rs",
    "src/leave.rs",
    "src/roster.rs",
    "docs/award.md",
];
const ARTIFACT_IDS: [&str; 3] = ["req_overtime", "topic_roster", "rule_penalty"];
const ARTIFACT_TYPES: [IdeationTargetType; 3] = [
    IdeationTargetType::Requirement,
    IdeationTargetType::Topic,
    IdeationTargetType::Rule,
];
const UNDISPOSED_STATES: [PromotionState; 2] = [PromotionState::Proposed, PromotionState::Asserted];
const DISPOSED_STATES: [PromotionState; 5] = [
    PromotionState::Accepted,
    PromotionState::Rejected,
    PromotionState::Deferred,
    PromotionState::Duplicate,
    PromotionState::Superseded,
];

struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(2_685_821_657_736_338_717).wrapping_add(1))
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 33).unwrap()
    }

    fn below(&mut self, bound: usize) -> usize {
        self.next_u32() as usize % bound
    }

    fn chance(&mut self) -> bool {
        self.next_u32().is_multiple_of(2)
    }
}

/// A generated case, described in terms of indices into the universes above so
/// the expectations can be computed without touching the built records.
struct Case {
    cited: BTreeSet<usize>,
    proposal_target: (usize, usize),
    touched: BTreeSet<usize>,
    demanded_targets: BTreeSet<(usize, usize)>,
}

impl Case {
    fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let cited = (0..FILES.len()).filter(|_| rng.chance()).collect();
        let proposal_target = (
            rng.below(ARTIFACT_TYPES.len()),
            rng.below(ARTIFACT_IDS.len()),
        );
        let touched = (0..FILES.len()).filter(|_| rng.chance()).collect();
        let mut demanded_targets = BTreeSet::new();
        for _ in 0..rng.below(3) {
            demanded_targets.insert((
                rng.below(ARTIFACT_TYPES.len()),
                rng.below(ARTIFACT_IDS.len()),
            ));
        }
        Self {
            cited,
            proposal_target,
            touched,
            demanded_targets,
        }
    }

    fn proposal(&self, promotion_state: PromotionState) -> ProposalCard {
        ProposalCard {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: StableId::new("proposal_generated").unwrap(),
            proposal_key: "generated".into(),
            proposal_type: ProposalType::RequirementCandidate,
            title: "Generated".into(),
            summary: "Generated".into(),
            confidence: None,
            traceability: ProposalTraceability {
                target: target(self.proposal_target),
                source_ids: Vec::new(),
                evidence_references: self
                    .cited
                    .iter()
                    .map(|&index| IdeationEvidenceReference {
                        reference_id: StableId::new(format!("evidence_{index}")).unwrap(),
                        evidence_type: IdeationEvidenceType::Artifact,
                        summary: "Code evidence".into(),
                        file_path: Some(FILES[index].into()),
                        line: None,
                    })
                    .collect(),
                supporting_claim_ids: Vec::new(),
            },
            builds_on: Vec::new(),
            promotion_state,
            duplicate_of: None,
            superseded_by: None,
        }
    }

    fn demand(&self) -> ProposalDemand {
        ProposalDemand::new(
            self.touched
                .iter()
                .map(|&index| FILES[index].into())
                .collect(),
            self.demanded_targets.iter().copied().map(target).collect(),
        )
    }

    /// A demand that matches everything the proposal claims, used to check that
    /// disposal wins over even a total match.
    fn saturating_demand(&self) -> ProposalDemand {
        ProposalDemand::new(
            self.cited
                .iter()
                .map(|&index| FILES[index].into())
                .collect(),
            vec![target(self.proposal_target)],
        )
    }

    fn overlapping_files(&self) -> BTreeSet<String> {
        self.cited
            .intersection(&self.touched)
            .map(|&index| FILES[index].to_owned())
            .collect()
    }

    fn territory_matches(&self) -> bool {
        self.demanded_targets.contains(&self.proposal_target)
    }
}

fn target((type_index, id_index): (usize, usize)) -> IdeationTarget {
    IdeationTarget {
        artifact_type: ARTIFACT_TYPES[type_index],
        artifact_id: StableId::new(ARTIFACT_IDS[id_index]).unwrap(),
    }
}

fn reported_paths(reasons: &[ProposalSurfaceReason]) -> BTreeSet<String> {
    reasons
        .iter()
        .filter_map(|reason| match reason {
            ProposalSurfaceReason::EvidenceSite { path } => Some(path.clone()),
            ProposalSurfaceReason::Territory { .. } => None,
        })
        .collect()
}

fn reported_targets(reasons: &[ProposalSurfaceReason]) -> Vec<IdeationTarget> {
    reasons
        .iter()
        .filter_map(|reason| match reason {
            ProposalSurfaceReason::EvidenceSite { .. } => None,
            ProposalSurfaceReason::Territory { target } => Some(target.clone()),
        })
        .collect()
}

#[test]
#[verifies("rule_proposal_surfacing", property)]
fn a_disposed_proposal_never_surfaces() {
    for seed in 0..CASES {
        let case = Case::generate(seed);
        for state in DISPOSED_STATES {
            let proposal = case.proposal(state);
            for demand in [case.demand(), case.saturating_demand()] {
                assert!(
                    surfacing_reasons(&proposal, &demand).is_empty(),
                    "seed {seed}: a {state:?} proposal surfaced"
                );
            }
        }
    }
}

#[test]
#[verifies("rule_proposal_surfacing", property)]
fn an_undisposed_proposal_surfaces_exactly_on_overlap() {
    let mut seen = Coverage::default();
    for seed in 0..CASES {
        let case = Case::generate(seed);
        let demand = case.demand();
        let overlapping_files = case.overlapping_files();
        let territory_matches = case.territory_matches();
        seen.record(!overlapping_files.is_empty(), territory_matches);
        for state in UNDISPOSED_STATES {
            let reasons = surfacing_reasons(&case.proposal(state), &demand);
            assert_eq!(
                !reasons.is_empty(),
                !overlapping_files.is_empty() || territory_matches,
                "seed {seed}: {state:?} proposal surfaced {} against \
                 files {overlapping_files:?} and territory {territory_matches}",
                !reasons.is_empty()
            );
            assert_eq!(
                reported_paths(&reasons),
                overlapping_files,
                "seed {seed}: reported evidence sites do not match the overlap"
            );
            assert_eq!(
                reported_targets(&reasons),
                if territory_matches {
                    vec![target(case.proposal_target)]
                } else {
                    Vec::new()
                },
                "seed {seed}: reported territory does not match the demand"
            );
        }
    }
    seen.assert_every_corner_was_generated();
}

/// Guard against a degenerate generator: a property that only ever sees one
/// corner of the input space proves nothing about the others.
#[derive(Default)]
struct Coverage {
    files_only: u32,
    territory_only: u32,
    both: u32,
    neither: u32,
}

impl Coverage {
    fn record(&mut self, files_overlap: bool, territory_matches: bool) {
        match (files_overlap, territory_matches) {
            (true, true) => self.both += 1,
            (true, false) => self.files_only += 1,
            (false, true) => self.territory_only += 1,
            (false, false) => self.neither += 1,
        }
    }

    fn assert_every_corner_was_generated(&self) {
        for (corner, count) in [
            ("file overlap only", self.files_only),
            ("territory match only", self.territory_only),
            ("both triggers", self.both),
            ("neither trigger", self.neither),
        ] {
            assert!(count > 0, "the generator never produced a {corner} case");
        }
    }
}

#[test]
#[verifies("rule_proposal_surfacing", property)]
fn no_surfacing_reason_is_fabricated() {
    for seed in 0..CASES {
        let case = Case::generate(seed);
        let demand = case.demand();
        let cited: BTreeSet<String> = case
            .cited
            .iter()
            .map(|&index| FILES[index].to_owned())
            .collect();
        let touched: BTreeSet<String> = case
            .touched
            .iter()
            .map(|&index| FILES[index].to_owned())
            .collect();
        let demanded: Vec<IdeationTarget> =
            case.demanded_targets.iter().copied().map(target).collect();
        for state in UNDISPOSED_STATES {
            for reason in surfacing_reasons(&case.proposal(state), &demand) {
                match reason {
                    ProposalSurfaceReason::EvidenceSite { path } => {
                        assert!(cited.contains(&path), "seed {seed}: {path} was never cited");
                        assert!(
                            touched.contains(&path),
                            "seed {seed}: {path} was never touched"
                        );
                    }
                    ProposalSurfaceReason::Territory { target } => {
                        assert!(
                            demanded.contains(&target),
                            "seed {seed}: territory {target:?} was never demanded"
                        );
                        assert_eq!(
                            target,
                            case.proposal(state).traceability.target,
                            "seed {seed}: territory names an artifact the proposal is not about"
                        );
                    }
                }
            }
        }
    }
}
