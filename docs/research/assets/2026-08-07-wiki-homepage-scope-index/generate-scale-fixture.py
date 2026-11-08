"""Generate the export JSON used for the homepage scale screenshots."""

import argparse
import json
from pathlib import Path

SCOPE = "homepage-scale"


def edge(kind, from_type, from_id, to_type, to_id):
    return {
        "scope_id": SCOPE,
        "id": f"edge_{kind}_{from_id}_{to_id}",
        "edge_type": kind,
        "from_type": from_type,
        "from_id": from_id,
        "to_type": to_type,
        "to_id": to_id,
    }


def build_fixture():
    domain_names = [
        "Identity", "Billing", "Privacy", "Security", "Reporting", "Operations",
        "Data retention", "Integrations", "Customer support", "Risk",
        "Accessibility", "Platform",
    ]
    sources = [{
        "scope_id": SCOPE,
        "id": f"source_{i:02d}",
        "name": f"Policy source {i:02d}",
        "source_type": "policy",
        "url": f"https://example.test/policy/{i:02d}",
        "reference": f"Policy volume {i:02d}",
    } for i in range(7)]
    domains = [{
        "scope_id": SCOPE,
        "id": f"domain_{i:02d}",
        "name": name,
        "description": f"Requirements and controls for {name.lower()}.",
    } for i, name in enumerate(domain_names)]
    requirements = [{
        "scope_id": SCOPE,
        "id": f"req_{i:03d}",
        "statement": (
            f"{domain_names[i % 12]} capability {i:03d} shall preserve "
            "traceable policy intent across implementation and review."
        ),
        "status": "active" if i % 9 else "discovery",
        "domain_id": f"domain_{i % 12:02d}",
        "source_refs": [] if i < 42 else [{
            "source_id": f"source_{i % 7:02d}",
            "clause": f"Section {i // 7 + 1}",
        }],
    } for i in range(228)]
    resolutions = [{
        "scope_id": SCOPE,
        "id": f"res_{i:03d}",
        "title": f"Implementation decision {i:03d}",
        "position": "Adopt the traceable implementation path.",
        "rationale": "Representative decision evidence for homepage-scale evaluation.",
        "status": "approved",
        "review_on": None,
    } for i in range(165)]
    rules = [{
        "scope_id": SCOPE,
        "id": f"rule_{i:03d}",
        "rule_code": f"CTRL-{i:03d}",
        "name": f"Control {i:03d}",
        "statement": f"Control {i:03d} shall enforce traceable policy intent.",
        "status": "active",
        "severity": ["low", "medium", "high", "critical"][i % 4],
        "rule_type": "business",
        "modality": "obligation",
        "source_document": "representative-corpus",
        "source_section": f"control-{i:03d}",
    } for i in range(576)]
    edges = [edge(
        "refines_into", "requirement", f"req_{i % 12:03d}",
        "requirement", f"req_{i:03d}",
    ) for i in range(12, 228)]
    edges += [edge(
        "resolves", "resolution", f"res_{i:03d}",
        "requirement", f"req_{i:03d}",
    ) for i in range(165)]
    edges += [edge(
        "produces", "resolution", f"res_{i % 165:03d}",
        "rule", f"rule_{i:03d}",
    ) for i in range(576)]
    return {
        "scope": SCOPE,
        "sources": sources,
        "domains": domains,
        "requirements": requirements,
        "topics": [], "questions": [],
        "resolutions": resolutions,
        "rules": rules,
        "services": [], "service_bindings": [],
        "edges": edges,
        "threads": [], "messages": [], "contributions": [],
        "synthesis_packets": [], "proposal_cards": [],
        "assertion_records": [], "dispositions": [],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(build_fixture(), indent=2))


if __name__ == "__main__":
    main()
