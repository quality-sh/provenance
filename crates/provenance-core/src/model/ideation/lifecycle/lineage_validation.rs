use super::AssertionRecord;
use crate::model::ProposalCard;
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

/// A proposal may not, through the assertions it builds on, end up building on
/// itself.
///
/// `builds_on` names assertions, and every assertion belongs to a proposal, so
/// a proposal's lineage is a walk: proposal to assertion to the proposal that
/// owns it, and on. The walk may never come back to where it started. One hop
/// counts: a proposal that builds on an assertion of its own is rejected like
/// any longer loop. Shared ancestors are not loops; a diamond, where two lines
/// of a proposal's lineage meet at one older assertion, is ordinary and passes.
///
/// The check is whole-graph, not per proposal. It runs over every proposal
/// handed to it and rejects the batch when any lineage closes on itself, even
/// one that the proposal a caller cares about cannot reach. The error names
/// the proposal where the walk closed; it does not print the path.
#[rule("rule_proposal_lineage_acyclic")]
pub(super) fn validate(
    proposals: &[ProposalCard],
    assertion_records: &[AssertionRecord],
) -> anyhow::Result<()> {
    let assertions = assertion_records
        .iter()
        .map(|assertion| (assertion.id.as_str(), assertion.proposal_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let edges = proposals
        .iter()
        .map(|proposal| {
            let ancestors = proposal
                .builds_on
                .iter()
                .map(|id| {
                    assertions.get(id.as_str()).copied().ok_or_else(|| {
                        anyhow::anyhow!("builds_on assertion {} does not exist", id.as_str())
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok((proposal.id.as_str(), ancestors))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for proposal in edges.keys() {
        visit(proposal, &edges, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit<'a>(
    proposal: &'a str,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> anyhow::Result<()> {
    if visited.contains(proposal) {
        return Ok(());
    }
    anyhow::ensure!(
        visiting.insert(proposal),
        "proposal assertion lineage contains a cycle at {proposal}"
    );
    for ancestor in edges.get(proposal).into_iter().flatten() {
        visit(ancestor, edges, visiting, visited)?;
    }
    visiting.remove(proposal);
    visited.insert(proposal);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate, AssertionRecord, ProposalCard};
    use crate::SUPPORTED_SCHEMA_VERSION;
    use provenance_macros::verifies;
    use serde_json::json;
    use std::collections::BTreeSet;

    const CASES: u64 = 200;

    /// Deterministic generator, so any failure reproduces from its seed alone.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }

        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            let drawn = self.next_u64() % u64::try_from(bound).unwrap();
            usize::try_from(drawn).unwrap()
        }
    }

    /// A generated builds-on structure, in indices rather than ids: what each
    /// proposal builds on, and which proposal each assertion belongs to.
    struct Graph {
        builds_on: Vec<Vec<usize>>,
        owner: Vec<usize>,
    }

    impl Graph {
        fn proposal_count(&self) -> usize {
            self.builds_on.len()
        }

        fn successors(&self, proposal: usize) -> Vec<usize> {
            self.builds_on[proposal]
                .iter()
                .map(|assertion| self.owner[*assertion])
                .collect()
        }
    }

    /// Which proposals each proposal can reach through its lineage, worked out
    /// by sweeping the generated structure directly. This is the oracle: it
    /// never calls the validator, and it collects nodes rather than colouring
    /// them, so it can agree with the validator only by being right.
    fn reachable(graph: &Graph) -> Vec<BTreeSet<usize>> {
        (0..graph.proposal_count())
            .map(|start| {
                let mut seen = BTreeSet::new();
                let mut pending = graph.successors(start);
                while let Some(next) = pending.pop() {
                    if seen.insert(next) {
                        pending.extend(graph.successors(next));
                    }
                }
                seen
            })
            .collect()
    }

    /// Proposals are laid out in a fixed order and may build only on
    /// assertions owned by earlier ones, which makes the result acyclic by
    /// construction. Some proposals own a second assertion, so two different
    /// assertion ids can lead back to one proposal.
    fn acyclic_graph(rng: &mut Rng) -> Graph {
        let proposal_count = 4 + rng.below(5);
        let mut owner: Vec<usize> = (0..proposal_count).collect();
        for _ in 0..rng.below(4) {
            let extra = rng.below(proposal_count);
            owner.push(extra);
        }
        let mut builds_on = vec![Vec::new(); proposal_count];
        for (proposal, chosen) in builds_on.iter_mut().enumerate().skip(1) {
            for (assertion, holder) in owner.iter().enumerate() {
                if *holder < proposal && chosen.len() < 3 && rng.below(2) == 0 {
                    chosen.push(assertion);
                }
            }
        }
        Graph { builds_on, owner }
    }

    /// An acyclic structure with one back edge added, closing a loop that runs
    /// through `focus`. When the focus has no lineage yet, the back edge is
    /// the one-hop case: the focus builds on an assertion of its own.
    fn cyclic_case(seed: u64) -> (Graph, usize) {
        let mut rng = Rng::new(seed);
        let mut graph = acyclic_graph(&mut rng);
        let focus = rng.below(graph.proposal_count());
        let ancestors: Vec<usize> = reachable(&graph)[focus].iter().copied().collect();
        let closes_at = if ancestors.is_empty() {
            focus
        } else {
            ancestors[rng.below(ancestors.len())]
        };
        // Assertion `focus` is the one proposal `focus` owns from the start.
        graph.builds_on[closes_at].push(focus);
        (graph, focus)
    }

    /// An acyclic structure with a loop closed between the last two proposals,
    /// which sit above `focus` in the layout and so are unreachable from it.
    fn remote_cycle_case(seed: u64) -> (Graph, usize, [usize; 2]) {
        let mut rng = Rng::new(seed ^ 0x5EED_C0DE);
        let mut graph = acyclic_graph(&mut rng);
        let last = graph.proposal_count() - 1;
        let second_last = last - 1;
        if !graph.builds_on[last].contains(&second_last) {
            graph.builds_on[last].push(second_last);
        }
        graph.builds_on[second_last].push(last);
        let focus = rng.below(second_last);
        (graph, focus, [second_last, last])
    }

    fn records(graph: &Graph) -> (Vec<ProposalCard>, Vec<AssertionRecord>) {
        let proposals = graph
            .builds_on
            .iter()
            .enumerate()
            .map(|(proposal, builds_on)| {
                let lineage: Vec<String> = builds_on
                    .iter()
                    .map(|assertion| format!("assertion_{assertion}"))
                    .collect();
                serde_json::from_value(json!({
                    "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default",
                    "id": format!("proposal_{proposal}"),
                    "proposal_key": format!("p{proposal}"), "proposal_type": "question",
                    "title": "generated", "summary": "generated",
                    "traceability": {
                        "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                        "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
                    },
                    "promotion_state": "proposed", "builds_on": lineage
                }))
                .unwrap()
            })
            .collect();
        let assertions = graph
            .owner
            .iter()
            .enumerate()
            .map(|(assertion, holder)| {
                serde_json::from_value(json!({
                    "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default",
                    "id": format!("assertion_{assertion}"),
                    "proposal_id": format!("proposal_{holder}"),
                    "synthesis_packet_id": "synthesis_a", "supporting_claim_ids": ["claim_a"]
                }))
                .unwrap()
            })
            .collect();
        (proposals, assertions)
    }

    /// The proposal index the rejection message points at.
    fn named_proposal(error: &str) -> usize {
        error
            .rsplit(' ')
            .next()
            .and_then(|id| id.strip_prefix("proposal_"))
            .unwrap_or_else(|| panic!("rejection named no proposal: {error}"))
            .parse()
            .unwrap()
    }

    /// The longest lineage a proposal has, in hops. Only meaningful on a
    /// structure the oracle has already called acyclic.
    fn longest_lineage(graph: &Graph) -> usize {
        let mut depth = vec![0_usize; graph.proposal_count()];
        for proposal in 0..graph.proposal_count() {
            depth[proposal] = graph
                .successors(proposal)
                .into_iter()
                .map(|ancestor| depth[ancestor] + 1)
                .max()
                .unwrap_or(0);
        }
        depth.into_iter().max().unwrap_or(0)
    }

    /// A proposal whose lineage splits and meets again at one older proposal.
    fn has_diamond(graph: &Graph, reach: &[BTreeSet<usize>]) -> bool {
        (0..graph.proposal_count()).any(|proposal| {
            let successors = graph.successors(proposal);
            successors.iter().enumerate().any(|(index, left)| {
                successors[index + 1..].iter().any(|right| {
                    left == right
                        || reach[*left].contains(right)
                        || reach[*right].contains(left)
                        || !reach[*left].is_disjoint(&reach[*right])
                })
            })
        })
    }

    #[test]
    #[verifies("rule_proposal_lineage_acyclic", property)]
    fn a_lineage_that_closes_on_itself_is_rejected_and_the_proposal_is_named() {
        let mut one_hop = 0_usize;
        for seed in 0..CASES {
            let (graph, focus) = cyclic_case(seed);
            let reach = reachable(&graph);
            assert!(
                reach[focus].contains(&focus),
                "seed {seed}: the injected back edge did not close a loop through proposal {focus}"
            );
            if graph.successors(focus).contains(&focus) {
                one_hop += 1;
            }
            let (proposals, assertions) = records(&graph);
            let error = validate(&proposals, &assertions).unwrap_err().to_string();
            let named = named_proposal(&error);
            assert!(
                reach[named].contains(&named)
                    && reach[named].contains(&focus)
                    && reach[focus].contains(&named),
                "seed {seed}: rejection named proposal {named}, which is not on the loop \
                 through proposal {focus}: {error}"
            );
        }
        assert!(
            one_hop > 0,
            "no generated case had a proposal building on an assertion of its own"
        );
    }

    #[test]
    #[verifies("rule_proposal_lineage_acyclic", property)]
    fn lineage_that_never_closes_passes_including_diamonds_and_long_chains() {
        let mut diamonds = 0_usize;
        let mut longest = 0_usize;
        for seed in 0..CASES {
            let mut rng = Rng::new(seed);
            let graph = acyclic_graph(&mut rng);
            let reach = reachable(&graph);
            assert!(
                (0..graph.proposal_count()).all(|proposal| !reach[proposal].contains(&proposal)),
                "seed {seed}: the generator produced a loop where it should not"
            );
            if has_diamond(&graph, &reach) {
                diamonds += 1;
            }
            longest = longest.max(longest_lineage(&graph));
            let (proposals, assertions) = records(&graph);
            validate(&proposals, &assertions)
                .unwrap_or_else(|error| panic!("seed {seed}: acyclic lineage rejected: {error}"));
        }
        assert!(diamonds > 0, "no generated case had a shared ancestor");
        assert!(
            longest >= 3,
            "the longest generated lineage was {longest} hops, too short to test chains"
        );
    }

    // What the code actually does, which is not what a per-proposal reading
    // would predict: the check is whole-graph, so a loop the focus proposal
    // cannot reach still rejects the batch the focus is in. The rejection then
    // names a proposal on the remote loop, never the focus.
    #[test]
    #[verifies("rule_proposal_lineage_acyclic", property)]
    fn a_loop_the_proposal_cannot_reach_still_rejects_the_batch() {
        for seed in 0..CASES {
            let (graph, focus, loop_members) = remote_cycle_case(seed);
            let reach = reachable(&graph);
            assert!(
                !reach[focus].contains(&focus),
                "seed {seed}: proposal {focus} was meant to stay off the loop"
            );
            assert!(
                loop_members
                    .iter()
                    .all(|member| !reach[focus].contains(member)),
                "seed {seed}: proposal {focus} can reach the loop, so it is not remote"
            );
            let (proposals, assertions) = records(&graph);
            let error = validate(&proposals, &assertions).unwrap_err().to_string();
            let named = named_proposal(&error);
            assert!(
                loop_members.contains(&named),
                "seed {seed}: rejection named proposal {named}, not a member of the remote \
                 loop {loop_members:?}: {error}"
            );
            assert_ne!(
                named, focus,
                "seed {seed}: rejection named the proposal that is off the loop"
            );
        }
    }
}
