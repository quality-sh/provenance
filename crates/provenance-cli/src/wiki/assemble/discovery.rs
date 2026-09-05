use crate::handlers::ScopeExport;
use crate::wiki::model::{
    DecisionIndexPage, DomainGroup, DomainIndexPage, DomainState, PageId, PageLink, RecordKind,
    RequirementPage, ResolutionPage, RulePage, SearchEntry, SearchIndexPage, SourcePage,
};
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

const MAX_SEARCH_EXAMPLE_CHARS: usize = 60;

struct RequirementRecord<'a> {
    id: &'a str,
    domain_id: Option<&'a str>,
    link: PageLink,
    statement: &'a str,
}

struct RuleRecord<'a> {
    link: PageLink,
    statement: &'a str,
    requirement_ids: Vec<&'a str>,
}

struct SearchCollection {
    kind: RecordKind,
    entries: Vec<SearchEntry>,
}

pub(super) fn build_discovery_pages(
    state: &ScopeExport,
    requirements: &[RequirementPage],
    resolutions: &[ResolutionPage],
    rules: &[RulePage],
    sources: &[SourcePage],
) -> (DomainIndexPage, SearchIndexPage, DecisionIndexPage) {
    let requirements = requirements
        .iter()
        .map(|page| RequirementRecord {
            id: &page.id.record_id,
            domain_id: page.domain_id.as_deref(),
            link: page_link(&page.id, &page.title),
            statement: &page.statement,
        })
        .collect::<Vec<_>>();
    let rules = rules
        .iter()
        .map(|page| RuleRecord {
            link: page_link(&page.id, &page.title),
            statement: &page.statement,
            requirement_ids: page
                .requirements
                .iter()
                .map(|link| link.target.record_id.as_str())
                .collect(),
        })
        .collect::<Vec<_>>();

    let decision_entries = resolutions
        .iter()
        .map(|page| SearchEntry {
            link: page_link(&page.id, &page.title),
            statement: page.position.clone(),
        })
        .collect::<Vec<_>>();
    let source_entries = sources
        .iter()
        .map(|page| SearchEntry {
            link: page_link(&page.id, &page.title),
            statement: page
                .reference
                .as_ref()
                .map(|reference| reference.label.clone())
                .or_else(|| page.url.clone())
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let search = search_index(
        &state.scope,
        &requirements,
        &decision_entries,
        &rules,
        &source_entries,
    );
    let decisions = DecisionIndexPage {
        scope: state.scope.clone(),
        title: "Decisions".to_string(),
        entries: decision_entries,
    };
    (
        domain_index(state, &requirements, &rules),
        search,
        decisions,
    )
}

/// Builds the offline index and names the record kinds it actually includes.
#[rule("rule_wiki_homepage_search_coverage")]
fn search_index(
    scope: &str,
    requirements: &[RequirementRecord<'_>],
    decisions: &[SearchEntry],
    rules: &[RuleRecord<'_>],
    sources: &[SearchEntry],
) -> SearchIndexPage {
    let collections = vec![
        SearchCollection {
            kind: RecordKind::Requirement,
            entries: requirements
                .iter()
                .map(|requirement| SearchEntry {
                    link: requirement.link.clone(),
                    statement: requirement.statement.to_string(),
                })
                .collect(),
        },
        SearchCollection {
            kind: RecordKind::Resolution,
            entries: decisions.to_vec(),
        },
        SearchCollection {
            kind: RecordKind::Rule,
            entries: rules
                .iter()
                .map(|rule| SearchEntry {
                    link: rule.link.clone(),
                    statement: rule.statement.to_string(),
                })
                .collect(),
        },
        SearchCollection {
            kind: RecordKind::Source,
            entries: sources.to_vec(),
        },
    ];
    let coverage = coverage_sentence(&collections);
    let entries: Vec<SearchEntry> = collections
        .into_iter()
        .flat_map(|collection| collection.entries)
        .collect();
    let example = entries
        .iter()
        .map(|entry| entry.link.title.trim())
        .find(|title| {
            !title.is_empty()
                && title.chars().count() <= MAX_SEARCH_EXAMPLE_CHARS
                && !title.ends_with('…')
                && !title.ends_with("...")
        })
        .map(str::to_string);
    SearchIndexPage {
        scope: scope.to_string(),
        title: "Search project records".to_string(),
        coverage,
        example,
        entries,
    }
}

fn coverage_sentence(collections: &[SearchCollection]) -> String {
    let labels = collections
        .iter()
        .map(|collection| match collection.kind {
            RecordKind::Requirement => "requirements",
            RecordKind::Resolution => "decisions",
            RecordKind::Rule => "rules",
            RecordKind::Source => "sources",
        })
        .collect::<Vec<_>>();
    let covered = match labels.as_slice() {
        [] => "nothing".to_string(),
        [only] => (*only).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, rest) = labels
                .split_last()
                .expect("a non-empty slice has a last item");
            format!("{}, and {last}", rest.join(", "))
        }
    };
    format!("Search covers {covered}.")
}

fn domain_index(
    state: &ScopeExport,
    requirements: &[RequirementRecord<'_>],
    rules: &[RuleRecord<'_>],
) -> DomainIndexPage {
    let authored_group_count = state.domains.len();
    let mut groups = Vec::new();
    let mut positions = BTreeMap::new();
    for domain in &state.domains {
        positions.insert(domain.id.as_str().to_string(), groups.len());
        groups.push(DomainGroup {
            state: DomainState::Defined {
                id: domain.id.as_str().to_string(),
                name: domain.name.clone(),
                description: domain.description.clone(),
            },
            requirements: Vec::new(),
            rules: Vec::new(),
        });
    }

    let requirement_domains = effective_requirement_domains(state, requirements);
    let missing_ids = requirement_domains
        .values()
        .flatten()
        .filter(|id| !positions.contains_key(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in missing_ids {
        positions.insert(id.clone(), groups.len());
        groups.push(DomainGroup {
            state: DomainState::Missing { id },
            requirements: Vec::new(),
            rules: Vec::new(),
        });
    }

    let needs_unassigned = requirement_domains.values().any(BTreeSet::is_empty)
        || rules.iter().any(|rule| {
            rule.requirement_ids.is_empty()
                || rule
                    .requirement_ids
                    .iter()
                    .any(|id| requirement_domains.get(*id).is_none_or(BTreeSet::is_empty))
        });
    let unassigned_position = needs_unassigned.then(|| {
        let position = groups.len();
        groups.push(DomainGroup {
            state: DomainState::Unassigned,
            requirements: Vec::new(),
            rules: Vec::new(),
        });
        position
    });

    for requirement in requirements {
        if let Some(domain_ids) = requirement_domains.get(requirement.id) {
            for domain_id in domain_ids {
                if let Some(position) = positions.get(domain_id) {
                    groups[*position]
                        .requirements
                        .push(requirement.link.clone());
                }
            }
            if domain_ids.is_empty() {
                if let Some(position) = unassigned_position {
                    groups[position].requirements.push(requirement.link.clone());
                }
            }
        }
    }
    for rule in rules {
        let mut group_positions = BTreeSet::new();
        for requirement_id in &rule.requirement_ids {
            match requirement_domains.get(*requirement_id) {
                Some(domain_ids) if !domain_ids.is_empty() => {
                    for domain_id in domain_ids {
                        if let Some(position) = positions.get(domain_id) {
                            group_positions.insert(*position);
                        }
                    }
                }
                Some(_) | None => {
                    if let Some(position) = unassigned_position {
                        group_positions.insert(position);
                    }
                }
            }
        }
        if rule.requirement_ids.is_empty() {
            if let Some(position) = unassigned_position {
                group_positions.insert(position);
            }
        }
        for position in group_positions {
            groups[position].rules.push(rule.link.clone());
        }
    }

    DomainIndexPage {
        scope: state.scope.clone(),
        title: "Requirements and rules by domain".to_string(),
        authored_group_count,
        groups,
        all_requirements: flat_requirement_entries(requirements),
        all_rules: flat_rule_entries(rules),
    }
}

fn flat_requirement_entries(requirements: &[RequirementRecord<'_>]) -> Vec<SearchEntry> {
    requirements
        .iter()
        .map(|requirement| SearchEntry {
            link: requirement.link.clone(),
            statement: requirement.statement.to_string(),
        })
        .collect()
}

fn flat_rule_entries(rules: &[RuleRecord<'_>]) -> Vec<SearchEntry> {
    rules
        .iter()
        .map(|rule| SearchEntry {
            link: rule.link.clone(),
            statement: rule.statement.to_string(),
        })
        .collect()
}

/// Decides which domains a requirement belongs to on the wiki.
///
/// A requirement belongs to its own domain and to every domain of every
/// requirement it refines, walking the `refines` chain all the way up. A
/// record whose chain names no domain gets an empty set here and is grouped
/// as Unassigned by the caller; it is never dropped from the index.
///
/// The walk visits each requirement at most once, so a cycle in `refines`
/// ends the walk instead of hanging.
#[rule("rule_domain_attribution")]
fn effective_requirement_domains(
    state: &ScopeExport,
    requirements: &[RequirementRecord<'_>],
) -> BTreeMap<String, BTreeSet<String>> {
    let records = requirements
        .iter()
        .map(|requirement| (requirement.id, requirement))
        .collect::<BTreeMap<_, _>>();
    let parents = state
        .requirements
        .iter()
        .filter_map(|requirement| {
            requirement
                .refines
                .as_ref()
                .map(|parent| (requirement.id.as_str(), parent.as_str()))
        })
        .collect::<BTreeMap<&str, &str>>();

    requirements
        .iter()
        .map(|requirement| {
            let mut domains = BTreeSet::new();
            let mut visited = BTreeSet::new();
            let mut pending = vec![requirement.id];
            while let Some(id) = pending.pop() {
                if !visited.insert(id) {
                    continue;
                }
                if let Some(record) = records.get(id) {
                    if let Some(domain_id) = record.domain_id {
                        domains.insert(domain_id.to_string());
                    }
                }
                if let Some(parent_id) = parents.get(id) {
                    pending.push(parent_id);
                }
            }
            (requirement.id.to_string(), domains)
        })
        .collect()
}

fn page_link(id: &PageId, title: &str) -> PageLink {
    PageLink {
        target: id.clone(),
        title: title.to_string(),
    }
}
