use crate::handlers::ScopeExport;
use serde::Deserialize;
use serde_json::json;

const PR_45_SCALE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/pr-45-homepage-scale.json"
));

#[derive(Deserialize)]
struct ScaleFixture {
    scope: String,
    counts: ScaleCounts,
    domains: Vec<ScaleDomain>,
    off_homepage_markers: OffHomepageMarkers,
}

#[derive(Deserialize)]
struct ScaleCounts {
    requirements: usize,
    decisions: usize,
    rules: usize,
    sources: usize,
    unfinished: usize,
}

#[derive(Deserialize)]
struct ScaleDomain {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct OffHomepageMarkers {
    search: String,
    domain: String,
    unfinished: String,
}

fn fixture_definition() -> ScaleFixture {
    serde_json::from_str(PR_45_SCALE).expect("PR 45 homepage scale fixture must be valid")
}

pub fn pr_45_scale_state() -> ScopeExport {
    let fixture = fixture_definition();
    let scope = fixture.scope.clone();
    let domain_names = fixture
        .domains
        .iter()
        .map(|domain| domain.name.as_str())
        .collect::<Vec<_>>();
    let sources = (0..fixture.counts.sources)
        .map(|number| {
            json!({
                "schema_version": 1, "scope_id": scope,
                "id": format!("source_{number:02}"), "name": format!("Policy source {number:02}"),
                "source_type": "policy", "url": format!("https://example.test/policy/{number:02}"),
                "reference": format!("Policy volume {number:02}"),
            })
        })
        .collect::<Vec<_>>();
    let domains = fixture.domains.iter().map(|domain| json!({
        "schema_version": 1, "scope_id": scope, "id": domain.id, "name": domain.name,
        "description": format!("Requirements and controls for {}.", domain.name.to_lowercase()),
    })).collect::<Vec<_>>();
    let requirements = (0..fixture.counts.requirements).map(|number| {
        let domain = number % domain_names.len();
        let source_refs = if number < fixture.counts.unfinished { vec![] } else { vec![json!({
            "source_id": format!("source_{:02}", number % fixture.counts.sources),
            "clause": format!("Section {}", number / fixture.counts.sources + 1),
        })] };
        let refines = (number >= domain_names.len())
            .then(|| json!(format!("req_{:03}", number % domain_names.len())));
        json!({
            "schema_version": 1, "scope_id": scope, "id": format!("req_{number:03}"),
            "statement": format!("{} capability {number:03} shall preserve traceable policy intent across implementation and review.", domain_names[domain]),
            "status": if number % 9 == 0 { "discovery" } else { "active" },
            "domain_id": format!("domain_{domain:02}"), "source_refs": source_refs,
            "refines": refines,
        })
    }).collect::<Vec<_>>();
    let resolutions = (0..fixture.counts.decisions)
        .map(|number| {
            json!({
                "schema_version": 1, "scope_id": scope, "id": format!("res_{number:03}"),
                "title": format!("Implementation decision {number:03}"),
                "position": "Adopt the traceable implementation path.",
                "rationale": "Representative decision evidence for homepage-scale evaluation.",
                "status": "approved", "review_on": null, "review_triggers": [],
                "requirement_ids": [format!("req_{number:03}")],
            })
        })
        .collect::<Vec<_>>();
    let rules = (0..fixture.counts.rules)
        .map(|number| {
            let severity = ["low", "medium", "high", "critical"][number % 4];
            json!({
                "schema_version": 1, "scope_id": scope, "id": format!("rule_{number:03}"),
                "rule_code": format!("CTRL-{number:03}"), "name": format!("Control {number:03}"),
                "statement": format!("Control {number:03} shall enforce traceable policy intent."),
                "status": "active", "severity": severity,
                "rule_type": "business", "modality": "obligation",
                "source_document": "representative-corpus", "source_section": format!("control-{number:03}"),
                "expression": {}, "inputs": [],
                "requirement_ids": [format!("req_{:03}", number % fixture.counts.decisions)],
                "resolution_ids": [format!("res_{:03}", number % fixture.counts.decisions)],
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "scope": fixture.scope, "sources": sources, "domains": domains,
        "requirements": requirements, "boundaries": [], "topics": [], "questions": [],
        "resolutions": resolutions, "rules": rules, "edges": [], "threads": [],
        "messages": [], "contributions": [], "synthesis_packets": [], "proposal_cards": [],
        "assertion_records": [], "dispositions": [],
    }))
    .expect("PR 45 generated records must remain a valid scope export")
}

pub fn pr_45_off_homepage_markers() -> [String; 3] {
    let markers = fixture_definition().off_homepage_markers;
    [markers.search, markers.domain, markers.unfinished]
}
