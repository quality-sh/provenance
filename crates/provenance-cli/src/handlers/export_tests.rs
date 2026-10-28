use super::{render_export, ScopeExport};
use crate::output::OutputFormat;
use provenance_core::{
    NodeType, ScopeId, StableId, Thread, ThreadParent, ThreadStatus, SUPPORTED_SCHEMA_VERSION,
};

fn thread(id: &str) -> Thread {
    Thread {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        parent: ThreadParent {
            node_type: NodeType::Rule,
            node_id: StableId::new("rule_export").unwrap(),
        },
        status: ThreadStatus::Active,
        created_at: 1,
    }
}

fn export_with_threads(threads: Vec<Thread>) -> ScopeExport {
    let mut exported: ScopeExport = serde_json::from_value(serde_json::json!({
        "scope": "default",
        "sources": [],
        "requirements": [],
        "resolutions": [],
        "rules": [],
        "threads": [],
        "messages": [],
    }))
    .unwrap();
    exported.threads = threads;
    exported
}

#[test]
fn jsonl_writes_one_line_for_each_record_of_each_collection() {
    let threads = vec![thread("thread_a"), thread("thread_b")];
    let rendered =
        render_export(OutputFormat::Jsonl, &export_with_threads(threads.clone())).unwrap();
    assert!(rendered.ends_with('\n'));
    let lines: Vec<serde_json::Value> = rendered
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let expected: Vec<serde_json::Value> = threads
        .iter()
        .map(|record| serde_json::to_value(record).unwrap())
        .collect();
    assert_eq!(lines, expected);
}

#[test]
fn json_output_reads_back_as_the_same_export() {
    let exported = export_with_threads(vec![thread("thread_a")]);
    let rendered = render_export(OutputFormat::Json, &exported).unwrap();
    assert!(rendered.ends_with("}\n"));
    let read_back: ScopeExport = serde_json::from_str(&rendered).unwrap();
    assert_eq!(read_back.scope, "default");
    assert_eq!(read_back.threads, exported.threads);
}

#[test]
fn the_summary_formats_count_each_collection() {
    let exported = export_with_threads(vec![thread("thread_a")]);
    assert_eq!(
        render_export(OutputFormat::Markdown, &exported).unwrap(),
        "# Provenance Export\n\n- Scope: default\n- Sources: 0\n- Domains: 0\n\
         - Requirements: 0\n- Boundaries: 0\n- Topics: 0\n- Questions: 0\n\
         - Resolutions: 0\n- Rules: 0\n- Proposals: 0\n"
    );
    assert_eq!(
        render_export(OutputFormat::Toon, &exported).unwrap(),
        "scope: default\nsources: 0\ndomains: 0\nrequirements: 0\nboundaries: 0\n\
         topics: 0\nquestions: 0\nresolutions: 0\nrules: 0\nproposals: 0\n"
    );
    assert_eq!(
        render_export(OutputFormat::Table, &exported).unwrap(),
        "scope\tsources\tdomains\trequirements\tboundaries\ttopics\tquestions\t\
         resolutions\trules\tproposals\ndefault\t0\t0\t0\t0\t0\t0\t0\t0\t0\n"
    );
}
