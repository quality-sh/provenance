//! Property tests for `rule_domain_attribution`.
//!
//! The graphs a wiki gets built from are unbounded, so the decision cannot be
//! checked by trying every input. These tests generate requirement trees with
//! sparse domain assignment and rules attached at random depths, then check
//! the assembled domain index against properties restated here from the
//! generator's own record of the graph, never by calling the assembler again.

use super::super::build_corpus;
use super::fixtures::{empty_state, requirement, resolution, rule, scope_id, sid};
use crate::handlers::ScopeExport;
use crate::wiki::links::LinkResolver;
use crate::wiki::model::{DomainGroup, DomainState, WikiCorpus};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{Domain, RequirementStatus};
use provenance_macros::verifies;
use std::collections::{BTreeMap, BTreeSet};

/// Deterministic generator: the same seed always builds the same graph, so a
/// failure names the case that produced it.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    fn next_bits(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("bound fits in u64");
        usize::try_from(self.next_bits() % bound).expect("a value below bound fits in usize")
    }
}

/// Two domain ids, one of them never declared, so generated graphs exercise
/// both the Defined and the Missing group states.
const DOMAIN_IDS: [&str; 2] = ["domain_declared", "domain_undeclared"];

struct GeneratedGraph {
    state: ScopeExport,
    /// Child requirement id to the requirements it refines from.
    parents: BTreeMap<String, Vec<String>>,
    /// Requirement id to the domain written on the record itself, if any.
    own_domain: BTreeMap<String, Option<String>>,
    /// Rule id to the requirements it was attached to, directly or through a
    /// resolution.
    attachments: BTreeMap<String, Vec<String>>,
    requirement_ids: Vec<String>,
    rule_ids: Vec<String>,
}

fn declared_domain() -> Domain {
    Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(DOMAIN_IDS[0]),
        name: "Declared domain".to_string(),
        description: Some("Domain with a record in the scope".to_string()),
        color: None,
    }
}

/// Builds a requirement tree (each requirement refines an earlier one, so
/// the generated shape is acyclic), assigns a domain to roughly a third of
/// the requirements, and hangs rules off requirements at random depths:
/// directly, through a resolution, or not at all.
fn generate(seed: u64) -> GeneratedGraph {
    let mut rng = Rng::new(seed);
    let mut state = empty_state();
    state.domains = vec![declared_domain()];

    let requirement_count = 2 + rng.below(7);
    let mut parents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut own_domain: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut requirement_ids = Vec::new();
    for index in 0..requirement_count {
        let id = format!("req_{index:02}");
        let domain = if rng.below(3) == 0 {
            Some(DOMAIN_IDS[rng.below(DOMAIN_IDS.len())].to_string())
        } else {
            None
        };
        let mut record = requirement(
            &id,
            &format!("Requirement {index} shall hold"),
            RequirementStatus::Active,
            vec![],
        );
        record.domain_id = domain.as_deref().map(sid);
        if index > 0 {
            let parent = format!("req_{:02}", rng.below(index));
            record.refines = Some(sid(&parent));
            parents.entry(id.clone()).or_default().push(parent);
        }
        state.requirements.push(record);
        own_domain.insert(id.clone(), domain);
        requirement_ids.push(id);
    }

    let rule_count = 1 + rng.below(4);
    let mut attachments: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rule_ids = Vec::new();
    for index in 0..rule_count {
        let rule_id = format!("rule_{index:02}");
        let mut generated = rule(&rule_id, None);
        let target = requirement_ids[rng.below(requirement_ids.len())].clone();
        match rng.below(3) {
            0 => {
                generated.requirement_ids = vec![sid(&target)];
                attachments.entry(rule_id.clone()).or_default().push(target);
            }
            1 => {
                let resolution_id = format!("res_{index:02}");
                let mut decision = resolution(&resolution_id, "Generated decision", vec![]);
                decision.requirement_ids = vec![sid(&target)];
                state.resolutions.push(decision);
                generated.resolution_ids = vec![sid(&resolution_id)];
                attachments.entry(rule_id.clone()).or_default().push(target);
            }
            _ => {
                attachments.insert(rule_id.clone(), Vec::new());
            }
        }
        state.rules.push(generated);
        rule_ids.push(rule_id);
    }

    GeneratedGraph {
        state,
        parents,
        own_domain,
        attachments,
        requirement_ids,
        rule_ids,
    }
}

impl GeneratedGraph {
    /// Domains carried by a requirement and everything it refines from,
    /// walked here from the generator's own parent map.
    fn chain_domains(&self, requirement_id: &str) -> BTreeSet<String> {
        let mut domains = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![requirement_id.to_string()];
        while let Some(id) = pending.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(Some(domain)) = self.own_domain.get(&id) {
                domains.insert(domain.clone());
            }
            if let Some(parent_ids) = self.parents.get(&id) {
                pending.extend(parent_ids.iter().cloned());
            }
        }
        domains
    }

    fn expected_rule_domains(&self, rule_id: &str) -> BTreeSet<String> {
        self.attachments
            .get(rule_id)
            .into_iter()
            .flatten()
            .flat_map(|requirement_id| self.chain_domains(requirement_id))
            .collect()
    }

    fn corpus(&self) -> WikiCorpus {
        build_corpus(&self.state, &LinkResolver::new(None))
    }
}

fn group_for_domain<'a>(corpus: &'a WikiCorpus, domain_id: &str) -> &'a DomainGroup {
    corpus
        .domains
        .groups
        .iter()
        .find(|group| match &group.state {
            DomainState::Defined { id, .. } | DomainState::Missing { id } => id == domain_id,
            DomainState::Unassigned => false,
        })
        .unwrap_or_else(|| panic!("domain index has no group for `{domain_id}`"))
}

fn grouped_rule_ids(corpus: &WikiCorpus) -> BTreeSet<&str> {
    corpus
        .domains
        .groups
        .iter()
        .flat_map(|group| group.rules.iter())
        .map(|link| link.target.record_id.as_str())
        .collect()
}

fn grouped_requirement_ids(corpus: &WikiCorpus) -> BTreeSet<&str> {
    corpus
        .domains
        .groups
        .iter()
        .flat_map(|group| group.requirements.iter())
        .map(|link| link.target.record_id.as_str())
        .collect()
}

fn assert_nothing_vanished(graph: &GeneratedGraph, corpus: &WikiCorpus, seed: u64) {
    let grouped_requirements = grouped_requirement_ids(corpus);
    for requirement_id in &graph.requirement_ids {
        assert!(
            grouped_requirements.contains(requirement_id.as_str()),
            "seed {seed}: requirement `{requirement_id}` is in no domain group"
        );
    }
    let grouped_rules = grouped_rule_ids(corpus);
    for rule_id in &graph.rule_ids {
        assert!(
            grouped_rules.contains(rule_id.as_str()),
            "seed {seed}: rule `{rule_id}` is in no domain group"
        );
    }
}

#[test]
#[verifies("rule_domain_attribution", property)]
fn rules_appear_in_every_domain_of_their_requirement_chain() {
    for seed in 0..128_u64 {
        let graph = generate(seed);
        let corpus = graph.corpus();
        for rule_id in &graph.rule_ids {
            let expected = graph.expected_rule_domains(rule_id);
            for domain_id in &expected {
                let group = group_for_domain(&corpus, domain_id);
                assert!(
                    group
                        .rules
                        .iter()
                        .any(|link| link.target.record_id == *rule_id),
                    "seed {seed}: rule `{rule_id}` missing from domain `{domain_id}`"
                );
            }
            if expected.is_empty() {
                let unassigned = corpus
                    .domains
                    .groups
                    .iter()
                    .find(|group| matches!(group.state, DomainState::Unassigned))
                    .unwrap_or_else(|| panic!("seed {seed}: no Unassigned group"));
                assert!(
                    unassigned
                        .rules
                        .iter()
                        .any(|link| link.target.record_id == *rule_id),
                    "seed {seed}: rule `{rule_id}` has no domain and is not Unassigned"
                );
            }
        }
    }
}

#[test]
#[verifies("rule_domain_attribution", property)]
fn every_requirement_and_rule_lands_in_some_group() {
    for seed in 0..128_u64 {
        let graph = generate(seed);
        let corpus = graph.corpus();
        assert_nothing_vanished(&graph, &corpus, seed);
    }
}

/// A cycle in `refines` is not a shape the CLI writes, but an imported or
/// hand-edited graph can hold one. Reaching the assertions at all is the
/// evidence that the walk terminates.
#[test]
#[verifies("rule_domain_attribution", property)]
fn a_refines_cycle_neither_hangs_nor_drops_records() {
    for seed in 0..128_u64 {
        let mut graph = generate(seed);
        if graph.requirement_ids.len() < 2 {
            continue;
        }
        let mut rng = Rng::new(seed ^ 0x5bf0_3635);
        let child_index = 1 + rng.below(graph.requirement_ids.len() - 1);
        let child = graph.requirement_ids[child_index].clone();
        let parent = graph.parents[&child][0].clone();
        let parent_record = graph
            .state
            .requirements
            .iter_mut()
            .find(|record| record.id.as_str() == parent)
            .expect("the parent is a generated requirement");
        parent_record.refines = Some(sid(&child));
        graph.parents.insert(parent.clone(), vec![child.clone()]);

        let corpus = graph.corpus();
        assert_nothing_vanished(&graph, &corpus, seed);
        for rule_id in &graph.rule_ids {
            for domain_id in &graph.expected_rule_domains(rule_id) {
                let group = group_for_domain(&corpus, domain_id);
                assert!(
                    group
                        .rules
                        .iter()
                        .any(|link| link.target.record_id == *rule_id),
                    "seed {seed}: cycle lost rule `{rule_id}` from domain `{domain_id}`"
                );
            }
        }
    }
}
