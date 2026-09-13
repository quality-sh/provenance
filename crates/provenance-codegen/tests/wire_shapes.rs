#[test]
fn rust_candidate_generates_actual_wire_shapes() {
    let document = provenance_codegen::corpus();
    let generated =
        provenance_codegen::rust_types(&document).expect("production corpus is supported");
    let text = generated.values().cloned().collect::<String>();
    for name in [
        "GetOutput",
        "EvidenceOutput",
        "PlanOutput",
        "VerificationRunOutput",
        "ReportOutput",
    ] {
        assert!(
            text.contains(&format!("pub struct {name}")),
            "missing concrete type {name}"
        );
    }
    assert!(text.contains("pub stale: ::std::option::Option<"));
    assert!(text.contains("pub node: ::std::option::Option<"));
    assert!(text.contains("pub findings: ::std::vec::Vec<"));
    for (name, content) in generated {
        assert!(content.lines().count() <= 500, "{name}");
    }
}
