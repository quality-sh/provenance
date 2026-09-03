//! Examples for `rule_reference_verified`: the four checks that decide
//! whether a pinned graph is handed back, and the one check that follows the
//! graph out of the repository.
//!
//! The last group of tests is about the exact-export document rather than the
//! primary implementation. A document has no repository behind it, so only one of the
//! four checks can travel with it: the digest. Those tests are unmarked,
//! because they never run the rule; what they hold in place is that the
//! document a verified reference produces carries the digest the rule
//! compared, and that reading it back re-takes that hash instead of trusting
//! it.
//!
//! The method is `examples`, never `exhaustion`. The four checks are a finite
//! branch set, but what each branch is fed is the state of a Git repository:
//! which commits it holds, which roots those commits descend from, and the
//! bytes under `.provenance/state` at each one. That domain is unbounded, so
//! an exhaustion claim here would be a false claim. What this suite does claim
//! is that no branch is unreached: each of the four has a case below that
//! lands on it, checked by the error that branch alone returns, and each is
//! built so the checks ahead of it pass.

use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_store::{
    graph_reference::{ExactExport, GraphReference, GraphReferenceError, GraphReferences},
    layout::ProvenanceLayout,
};
use serde_json::{json, Value};

const AUTHOR: [&str; 4] = [
    "-c",
    "user.name=Graph Reference Test",
    "-c",
    "user.email=graph-reference@example.invalid",
];

fn git(repo: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn write_manifest(repo: &std::path::Path, path_prefix: &str) {
    let root = camino::Utf8PathBuf::from_path_buf(repo.to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new(path_prefix),
        ))
        .unwrap(),
    )
    .unwrap();
}

fn commit_all(repo: &std::path::Path, message: &str) -> String {
    git(repo, &["add", ".provenance/state"]);
    let mut args = AUTHOR.to_vec();
    args.extend(["commit", "-qm", message]);
    git(repo, &args);
    git(repo, &["rev-parse", "HEAD"])
}

/// A repository holding one committed canonical graph.
fn committed_store() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    write_manifest(temp.path(), ".");
    git(temp.path(), &["init", "-q"]);
    commit_all(temp.path(), "canonical graph");
    temp
}

/// The same, with one source record in it, so a document cut from it has a
/// record that can be edited without touching anything around it.
fn committed_store_with_a_record(name: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    write_manifest(temp.path(), ".");
    let sources = temp.path().join(".provenance/state/scopes/default/sources");
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(
        sources.join("source.jsonl"),
        serde_json::to_vec(&json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "source_retention",
            "name": name,
            "source_type": "policy",
            "url": null
        }))
        .unwrap(),
    )
    .unwrap();
    git(temp.path(), &["init", "-q"]);
    commit_all(temp.path(), "canonical graph with one record");
    temp
}

fn references(temp: &tempfile::TempDir) -> GraphReferences {
    GraphReferences::open(camino::Utf8Path::from_path(temp.path()).unwrap()).unwrap()
}

fn issue(temp: &tempfile::TempDir) -> GraphReference {
    references(temp).issue("default", None, None).unwrap()
}

/// Reads a refusal back as the field it named, what the reference claimed, and
/// what the repository actually holds. Every read verb is asked, because the
/// rule gates all three and a verb that skipped it would show up here.
fn refusal(temp: &tempfile::TempDir, reference: &GraphReference) -> (&'static str, String, String) {
    let references = references(temp);
    let refused = [
        references.show(reference).err(),
        references.verify(reference).err(),
        references.exact_export(reference).err(),
    ];
    for (verb, error) in ["show", "verify", "exact-export"].iter().zip(&refused) {
        assert!(
            error.is_some(),
            "{verb} honoured a reference it should refuse"
        );
        assert_eq!(
            error, &refused[0],
            "{verb} did not refuse the reference the way show did"
        );
    }
    match &refused[0] {
        Some(GraphReferenceError::Mismatched {
            field,
            expected,
            actual,
        }) => (*field, expected.clone(), actual.clone()),
        other => panic!("show should have refused with a mismatch, got {other:?}"),
    }
}

#[test]
#[verifies("rule_reference_verified", examples)]
fn honours_the_reference_it_issued() {
    let temp = committed_store();
    let reference = issue(&temp);
    let references = references(&temp);

    // The positive control. Without it the refusals below prove nothing: a
    // rule that refused everything would pass every other test in the file.
    assert_eq!(references.show(&reference).unwrap().reference, reference);
    assert!(references.verify(&reference).unwrap().valid);
    assert_eq!(
        references.exact_export(&reference).unwrap().graph.scope.id,
        ScopeId::new("default").unwrap()
    );
}

/// First check, absent commit. A reference cut from history this clone never
/// received, or history that was rewritten away, has nothing behind it.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_a_commit_this_repository_does_not_hold() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    reference.commit = "0".repeat(40);

    let references = references(&temp);
    for error in [
        references.show(&reference).unwrap_err(),
        references.verify(&reference).unwrap_err(),
        references.exact_export(&reference).unwrap_err(),
    ] {
        let GraphReferenceError::Missing { detail } = error else {
            panic!("an absent commit should be Missing, got {error:?}");
        };
        assert!(
            detail.contains(&reference.commit),
            "refusal should name the commit that is gone: {detail}"
        );
    }
}

/// First check, wrong object. The reference must name the commit itself, not
/// something that resolves to it. An annotated tag id is 40 hex digits like a
/// commit id and passes the well-formedness rule, but it peels to the commit
/// it points at, so it is a name for a graph rather than the graph's commit.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_an_object_that_is_not_the_commit_it_claims() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    let mut args = AUTHOR.to_vec();
    args.extend(["tag", "-a", "handoff", "-m", "pinned handoff"]);
    git(temp.path(), &args);
    let tag_object = git(temp.path(), &["rev-parse", "handoff"]);
    assert_ne!(
        tag_object, reference.commit,
        "the tag must be its own object"
    );
    let commit = reference.commit.clone();
    reference.commit = tag_object.clone();

    let (field, expected, actual) = refusal(&temp, &reference);
    assert_eq!(field, "commit");
    assert_eq!(expected, tag_object);
    assert_eq!(actual, commit);
}

/// Second check, repository identity. The identity is the hash of the root
/// commits the pinned commit descends from, so it separates two repositories
/// that share a store path and a scope name.
///
/// The field is edited here rather than issued elsewhere. A reference from
/// another repository names that repository's commits, which this one does not
/// hold, so it would be caught by the check ahead of this one; nothing outside
/// this branch can build a commit that exists here and carries a different
/// root. The edit is what a forged or hand-patched reference looks like.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_a_reference_belonging_to_another_repository() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    let genuine = reference.repository_id.clone();
    let claimed = format!("git1_{}", "b".repeat(64));
    reference.repository_id = claimed.clone();

    let (field, expected, actual) = refusal(&temp, &reference);
    assert_eq!(field, "repository_id");
    assert_eq!(expected, claimed);
    assert_eq!(actual, genuine);
}

/// Third check, the digest. The record at the pinned commit cannot be mutated
/// where it lies: change one byte under a commit and Git gives you a different
/// commit id, so the pin still points at the old bytes. The honest way to make
/// the graph at the named commit differ from the recorded digest is to move
/// the reference onto a later commit that carries a changed graph, which is
/// what this does. The first two checks still pass there: the commit exists
/// and descends from the same root.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_a_commit_whose_graph_does_not_hash_to_the_recorded_digest() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    write_manifest(temp.path(), "src");
    let changed = commit_all(temp.path(), "change the canonical graph");
    reference.commit = changed;

    let (field, expected, actual) = refusal(&temp, &reference);
    assert_eq!(field, "graph_digest");
    assert_eq!(expected, reference.graph_digest);
    assert_ne!(actual, reference.graph_digest);
    assert!(actual.starts_with("sha256:"));
}

/// Third check again, from the other side: the recorded digest is edited while
/// the graph stays put. Same branch, and the pair is what tells a holder the
/// digest is compared rather than copied.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_a_digest_that_was_written_over() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    let genuine = reference.graph_digest.clone();
    reference.graph_digest = format!("sha256:{}", "0".repeat(64));

    let (field, expected, actual) = refusal(&temp, &reference);
    assert_eq!(field, "graph_digest");
    assert_eq!(expected, reference.graph_digest);
    assert_eq!(actual, genuine);
}

/// Fourth check, the id. Two commits over the same graph share a digest and
/// differ only in their reference id, so an id lifted from one and pasted onto
/// the other clears every check ahead of this one: the commit is here, the
/// repository is right, the graph hashes as recorded. Only recomputing the id
/// over the parts catches it.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_an_id_minted_over_other_parts() {
    let temp = committed_store();
    let first = issue(&temp);
    let mut args = AUTHOR.to_vec();
    args.extend(["commit", "--allow-empty", "-qm", "later handoff commit"]);
    git(temp.path(), &args);
    let mut second = issue(&temp);

    assert_eq!(first.graph_digest, second.graph_digest);
    assert_ne!(first.reference_id, second.reference_id);
    let genuine = second.reference_id.clone();
    second.reference_id = first.reference_id.clone();

    let (field, expected, actual) = refusal(&temp, &second);
    assert_eq!(field, "reference_id");
    assert_eq!(expected, first.reference_id);
    assert_eq!(actual, genuine);
}

/// The document a verified reference produces, as JSON ready to be edited.
fn exported_document(temp: &tempfile::TempDir, reference: &GraphReference) -> Value {
    serde_json::to_value(references(temp).exact_export(reference).unwrap()).unwrap()
}

/// Reads a document refusal back as the digest the document claimed and the
/// digest its graph actually hashes to, insisting the error names both.
fn document_refusal(document: &Value) -> (String, String) {
    let bytes = serde_json::to_vec(document).unwrap();
    let error = match ExactExport::from_json(&bytes) {
        Err(error) => error,
        Ok(export) => panic!("a tampered document was read back as {export:?}"),
    };
    let GraphReferenceError::Mismatched {
        field,
        expected,
        actual,
    } = &error
    else {
        panic!("a tampered document should be refused as a mismatch, got {error:?}");
    };
    assert_eq!(*field, "graph_digest");
    let text = error.to_string();
    assert!(
        text.contains(expected) && text.contains(actual),
        "the refusal must name both digests: {text}"
    );
    assert_ne!(expected, actual);
    (expected.clone(), actual.clone())
}

/// The document a verified reference hands out is readable back as itself,
/// with no repository in the way.
///
/// This is the positive control for the three refusals below, and it is where
/// the online and offline halves meet: `exact_export` goes through the rule,
/// which recomputes the digest from the pinned commit, and the digest it
/// compared is the one written into the document. Without this, a check that
/// refused every document would pass the rest of the file.
#[test]
#[verifies("rule_reference_verified", examples)]
fn hands_out_a_document_that_verifies_itself_offline() {
    let temp = committed_store_with_a_record("Retention policy");
    let reference = issue(&temp);
    let export = references(&temp).exact_export(&reference).unwrap();

    assert_eq!(export.graph_digest, reference.graph_digest);
    let read_back = ExactExport::from_json(&serde_json::to_vec(&export).unwrap()).unwrap();
    assert_eq!(read_back, export);
    assert_eq!(read_back.reference_id, reference.reference_id);
}

/// The whole graph object swapped for another one. Both documents are genuine
/// exports of graphs this repository really holds, so every field of the
/// document except the graph is untouched and well formed; only recomputing
/// the digest over the graph that arrived separates them.
#[test]
fn refuses_a_document_whose_graph_was_swapped_for_another() {
    let temp = committed_store_with_a_record("Retention policy");
    let pinned = issue(&temp);
    let mut document = exported_document(&temp, &pinned);
    write_manifest(temp.path(), "src");
    commit_all(temp.path(), "a different canonical graph");
    let other = issue(&temp);
    let other_document = exported_document(&temp, &other);
    assert_ne!(pinned.graph_digest, other.graph_digest);

    document["graph"] = other_document["graph"].clone();
    let (claimed, hashed) = document_refusal(&document);
    assert_eq!(claimed, pinned.graph_digest);
    assert_eq!(hashed, other.graph_digest);
}

/// One record inside the graph edited, and nothing else. The document still
/// names the reference it came from and still carries a well-formed digest;
/// the digest is simply no longer the digest of what is in it.
#[test]
fn refuses_a_document_with_one_record_edited() {
    let temp = committed_store_with_a_record("Retention policy");
    let reference = issue(&temp);
    let mut document = exported_document(&temp, &reference);

    assert_eq!(document["graph"]["sources"][0]["name"], "Retention policy");
    document["graph"]["sources"][0]["name"] = json!("Retention policy (superseded)");
    let (claimed, hashed) = document_refusal(&document);
    assert_eq!(claimed, reference.graph_digest);
    assert!(hashed.starts_with("sha256:"));
}

/// The digest field written over while the graph stays put: the other side of
/// the same check, and the pair is what tells a holder the digest is taken
/// again rather than read out.
#[test]
fn refuses_a_document_whose_digest_was_written_over() {
    let temp = committed_store_with_a_record("Retention policy");
    let reference = issue(&temp);
    let mut document = exported_document(&temp, &reference);

    let forged = format!("sha256:{}", "0".repeat(64));
    document["graph_digest"] = json!(forged);
    let (claimed, hashed) = document_refusal(&document);
    assert_eq!(claimed, forged);
    assert_eq!(hashed, reference.graph_digest);
}

/// The scope id, which no check compares on its own. It is hashed into the
/// reference id and it decides which graph gets projected, so a renamed
/// reference is refused either because the pinned manifest has no such scope,
/// as here, or because the scope it does name projects a different graph and
/// fails the digest. Nothing about it is taken from the reference and trusted.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refuses_a_reference_renamed_onto_another_scope() {
    let temp = committed_store();
    let mut reference = issue(&temp);
    reference.scope_id = "other".to_string();

    let references = references(&temp);
    let error = references.verify(&reference).unwrap_err();
    let GraphReferenceError::Missing { detail } = error else {
        panic!("an absent scope should be Missing, got {error:?}");
    };
    assert!(
        detail.contains("other"),
        "refusal should name the scope that is not in the pinned manifest: {detail}"
    );
}
