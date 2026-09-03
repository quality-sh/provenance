//! Property tests for `rule_asserted_evidence_immutable`.
//!
//! Each world is generated from a seed with citedness fixed by construction:
//! the generator decides first which contributions and packets an assertion
//! will cite, then writes assertions that cite exactly those. Nothing here
//! recomputes citedness the way the rule does, so the expectations are
//! independent of the matching code under test.

use crate::state_store::ideation_batches::{
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged,
};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    AssertionId, AssertionRecord, Contribution, ContributionStance, IdeationEvidenceType,
    IdeationTarget, IdeationTargetType, MaterialClaim, ScopeId, StableId, SynthesisPacket,
    UncertaintyLevel, UncertaintyRating,
};
use provenance_macros::verifies;

const WORLDS: u64 = 256;

struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        // xorshift64 needs a non-zero state; the low bit carries no signal, so
        // the output below mixes the state rather than returning it.
        Self(seed.wrapping_mul(6_364_136_223_846_793_005) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        state.wrapping_mul(2_685_821_657_736_338_717)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn chance(&mut self, in_n: u64) -> bool {
        self.below(in_n) == 0
    }
}

/// A generated world plus the citedness the generator built into it.
struct World {
    contributions: Vec<Contribution>,
    packets: Vec<SynthesisPacket>,
    assertions: Vec<AssertionRecord>,
    /// Indexes into `contributions` that the generator made an assertion cite.
    cited_contributions: Vec<usize>,
    /// Indexes into `packets` that the generator made an assertion cite.
    cited_packets: Vec<usize>,
}

impl World {
    fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let scope = ScopeId::new("default").unwrap();
        let contribution_count = rng.below(3) + 1;
        let packet_count = rng.below(3) + 1;

        let mut contributions = Vec::new();
        let mut cited_contributions = Vec::new();
        let mut cited_claims = Vec::new();
        for index in 0..contribution_count {
            let cited = rng.chance(2);
            // A cited contribution needs at least one claim to be cited through.
            let claim_count = if cited {
                rng.below(2) + 1
            } else {
                rng.below(3)
            };
            let claims = (0..claim_count)
                .map(|claim| claim_id(index, claim))
                .collect::<Vec<_>>();
            if cited {
                cited_contributions.push(usize::try_from(index).unwrap());
                cited_claims.extend(claims.clone());
            }
            contributions.push(contribution(&scope, index, &claims));
        }

        let mut packets = Vec::new();
        let mut cited_packets = Vec::new();
        let mut cited_packet_ids = Vec::new();
        for index in 0..packet_count {
            let cited = rng.chance(2);
            if cited {
                cited_packets.push(usize::try_from(index).unwrap());
                cited_packet_ids.push(packet_id(index));
            }
            packets.push(packet(&scope, index));
        }

        let assertions = assertions_citing(
            &mut rng,
            &scope,
            &cited_claims,
            &cited_packet_ids,
            packet_count,
        );
        Self {
            contributions,
            packets,
            assertions,
            cited_contributions,
            cited_packets,
        }
    }

    fn uncited_contributions(&self) -> Vec<usize> {
        (0..self.contributions.len())
            .filter(|index| !self.cited_contributions.contains(index))
            .collect()
    }

    fn uncited_packets(&self) -> Vec<usize> {
        (0..self.packets.len())
            .filter(|index| !self.cited_packets.contains(index))
            .collect()
    }
}

/// Writes the assertions that cite exactly the given claims and packets, plus
/// noise assertions that cite ids belonging to nothing.
fn assertions_citing(
    rng: &mut Rng,
    scope: &ScopeId,
    cited_claims: &[StableId],
    cited_packet_ids: &[StableId],
    packet_count: u64,
) -> Vec<AssertionRecord> {
    let mut assertions = Vec::new();
    // One assertion per cited packet, carrying a share of the cited claims so
    // that citing an id never depends on which assertion it landed on.
    for (index, packet) in cited_packet_ids.iter().enumerate() {
        assertions.push(assertion(scope, index, packet, Vec::new()));
    }
    for (index, claim) in cited_claims.iter().enumerate() {
        let packet = cited_packet_ids
            .get(index % cited_packet_ids.len().max(1))
            .cloned()
            .unwrap_or_else(|| StableId::new("synthesis_absent").unwrap());
        assertions.push(assertion(
            scope,
            assertions.len(),
            &packet,
            vec![claim.clone()],
        ));
    }
    if rng.chance(2) {
        // Noise: an assertion whose evidence ids match no generated record.
        // It must not freeze anything.
        assertions.push(assertion(
            scope,
            assertions.len(),
            &packet_id(packet_count + 7),
            vec![claim_id(99, 99)],
        ));
    }
    assertions
}

fn claim_id(contribution: u64, claim: u64) -> StableId {
    StableId::new(format!("claim_{contribution}_{claim}")).unwrap()
}

fn packet_id(index: u64) -> StableId {
    StableId::new(format!("synthesis_{index}")).unwrap()
}

fn target() -> IdeationTarget {
    IdeationTarget {
        artifact_type: IdeationTargetType::Requirement,
        artifact_id: StableId::new("req_overtime").unwrap(),
    }
}

fn contribution(scope: &ScopeId, index: u64, claims: &[StableId]) -> Contribution {
    Contribution {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new(format!("contribution_{index}")).unwrap(),
        target: target(),
        participant_slot: "reviewer".into(),
        stance: ContributionStance::Support,
        strongest_finding: format!("finding {index}"),
        evidence_references: Vec::new(),
        material_claims: claims
            .iter()
            .map(|claim| MaterialClaim {
                claim_id: claim.clone(),
                statement: format!("claim {}", claim.as_str()),
                evidence_type: IdeationEvidenceType::Source,
                evidence_reference_ids: Vec::new(),
                confidence: None,
            })
            .collect(),
        risks: Vec::new(),
        objections: Vec::new(),
        challenges: Vec::new(),
        suggested_artifact_changes: Vec::new(),
        unsupported_recommendations: Vec::new(),
        uncertainty: UncertaintyRating {
            level: UncertaintyLevel::Low,
            rationale: "Direct".into(),
        },
        open_questions: Vec::new(),
    }
}

fn packet(scope: &ScopeId, index: u64) -> SynthesisPacket {
    SynthesisPacket {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: packet_id(index),
        target: target(),
        summary: format!("summary {index}"),
        consensus: Vec::new(),
        contested_claims: Vec::new(),
        minority_objections: Vec::new(),
        evidence_gaps: Vec::new(),
        unsupported_speculation: Vec::new(),
        open_questions: Vec::new(),
        suggested_artifacts: Vec::new(),
        required_human_decisions: Vec::new(),
    }
}

fn assertion(
    scope: &ScopeId,
    index: usize,
    packet_id: &StableId,
    claims: Vec<StableId>,
) -> AssertionRecord {
    AssertionRecord {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: AssertionId::new(format!("assertion_{index}")).unwrap(),
        proposal_id: StableId::new(format!("proposal_{index}")).unwrap(),
        synthesis_packet_id: packet_id.clone(),
        supporting_claim_ids: claims,
    }
}

/// Every kind of edit a replacement can carry, none of which touches the id.
fn altered(contribution: &Contribution) -> Contribution {
    let mut altered = contribution.clone();
    altered.strongest_finding.push_str(" (edited)");
    altered.material_claims.pop();
    altered.open_questions.push("late doubt".into());
    altered
}

fn altered_packet(packet: &SynthesisPacket) -> SynthesisPacket {
    let mut altered = packet.clone();
    altered.summary.push_str(" (edited)");
    altered.open_questions.push("late doubt".into());
    altered
}

#[test]
#[verifies("rule_asserted_evidence_immutable", property)]
fn altered_asserted_evidence_is_rejected_naming_the_record() {
    for seed in 0..WORLDS {
        let world = World::generate(seed);
        for &index in &world.cited_contributions {
            let existing = &world.contributions[index];
            let error = ensure_asserted_contribution_unchanged(
                existing,
                &altered(existing),
                &world.assertions,
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(existing.id.as_str())
                    && error.contains("referenced by an assertion"),
                "seed {seed}: {error}"
            );
        }
        for &index in &world.cited_packets {
            let existing = &world.packets[index];
            let error = ensure_asserted_synthesis_unchanged(
                existing,
                &altered_packet(existing),
                &world.assertions,
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(existing.id.as_str())
                    && error.contains("referenced by an assertion"),
                "seed {seed}: {error}"
            );
        }
    }
}

#[test]
#[verifies("rule_asserted_evidence_immutable", property)]
fn rewriting_evidence_with_identical_content_is_accepted() {
    for seed in 0..WORLDS {
        let world = World::generate(seed);
        for existing in &world.contributions {
            ensure_asserted_contribution_unchanged(existing, &existing.clone(), &world.assertions)
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        }
        for existing in &world.packets {
            ensure_asserted_synthesis_unchanged(existing, &existing.clone(), &world.assertions)
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        }
    }
}

#[test]
#[verifies("rule_asserted_evidence_immutable", property)]
fn evidence_behind_no_assertion_may_change_freely() {
    for seed in 0..WORLDS {
        let world = World::generate(seed);
        for index in world.uncited_contributions() {
            let existing = &world.contributions[index];
            ensure_asserted_contribution_unchanged(existing, &altered(existing), &world.assertions)
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        }
        for index in world.uncited_packets() {
            let existing = &world.packets[index];
            ensure_asserted_synthesis_unchanged(
                existing,
                &altered_packet(existing),
                &world.assertions,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        }
    }
}

#[test]
fn generated_worlds_cover_both_cited_and_uncited_evidence() {
    let mut cited = 0;
    let mut uncited = 0;
    for seed in 0..WORLDS {
        let world = World::generate(seed);
        cited += world.cited_contributions.len() + world.cited_packets.len();
        uncited += world.uncited_contributions().len() + world.uncited_packets().len();
    }
    assert!(cited > 100, "generator produced only {cited} cited records");
    assert!(
        uncited > 100,
        "generator produced only {uncited} uncited records"
    );
}
