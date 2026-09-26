//! Domain guidance shared by CLI output and MCP discovery.

use crate::action::{self, Action};
use serde::Serialize;
use std::fmt::Write;

pub const GET_DESCRIPTION: &str =
    "Read one repository record by its repository-local ID. Select its record, children, grounding, or impact view.";
pub const SEARCH_DESCRIPTION: &str = "Find records across the permitted kinds in the bound scope.";
pub const CHECK_DESCRIPTION: &str =
    "Check graph validity, statement quality, and binding coverage.";

/// The same domain text in structured CLI output and native MCP instructions.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Guidance {
    pub guidance: String,
}

/// Produce guidance without a repository, a graph query, or a cache.
pub fn guidance() -> Guidance {
    let mut text = String::from(
        "# Provenance\n\n\
Provenance records requirements, decisions, and the rules that connect them to code.\n\n\
## Domain\n\n\
- Requirement: an obligation the system must satisfy. Requirements can refine other Requirements.\n\
- Rule: an identified atomic behavioural obligation that refines one or more Requirements. A Rule can exist before code or verification.\n\
- Resolution: a recorded decision that removes ambiguity. A Resolution can produce a Rule when it establishes a precise obligation.\n\
- Source: identified evidence that graph records can cite. A citation records where an obligation came from.\n\
- Boundary: an explicit limit attached to a Requirement.\n\
- Topic: a claimable shaping work area attached to a Requirement.\n\
- Question: a concern to resolve, attached to a Topic and a Requirement, with a selected resolution method.\n\
- Discussion: one concern identified by a root Message and its replies inside a Thread.\n\n\
## Code and evidence\n\n\
An Implementation binding connects a Rule to production code that realizes it.\n\
Verification is evidence that a Rule holds. A Verification binding identifies intended evidence; a Verification run records one execution and its result.\n\
Rule lifecycle, decision grounding, implementation, and verification are separate facts. An active Rule can precede its implementation.\n\
Unimplemented and Unverified describe absent bindings or evidence; they are not stored Rule statuses. A graph read does not scan code or establish a coverage verdict.\n\
A source citation does not count as an Implementation binding. Coverage checks evaluate code bindings separately from graph reads.\n\n\
## Actions\n\n\
Use search to find IDs, then get to read the selected record and its related views. Create names an explicit record type; actions on an existing record infer its type.\n\
Available actions depend on the selected surface and its access. Use CLI help or the advertised MCP tools for supported inputs and actions.\n\n",
    );
    for (name, description) in [
        ("get", GET_DESCRIPTION),
        ("search", SEARCH_DESCRIPTION),
        ("check", CHECK_DESCRIPTION),
        ("api", crate::api::API_DESCRIPTION),
    ] {
        writeln!(text, "- {name}: {description}").expect("writing to a String cannot fail");
    }
    for action in Action::RECORD.into_iter().chain(Action::DISCUSSION) {
        writeln!(
            text,
            "- {}: {}",
            action.as_str(),
            action::description(action)
        )
        .expect("writing to a String cannot fail");
    }
    text.pop();
    Guidance { guidance: text }
}
