//! L6: the reads after a write equal the reads after a full rebuild.
//!
//! The read list holds each list read of the transcript and the record
//! reads of the simulation fixture: the Requirements of the initial state,
//! each Requirement that the case created, the Rule and the Topic. So each
//! record read has a record to read, and a read that finds no record breaks
//! the law.

use crate::process::{now, Reads};
use crate::support::transcript::{assert_same, copy_without_projection, read_paths, READS};
use provenance_simulation::generate::{DETAIL_REQUIREMENT, FIRST_REQUIREMENT, FIRST_RULE, TOPIC};
use provenance_simulation::report;
use provenance_transport::fixture::records::Repository;
use serde_json::Value;

/// The reads of L6 for a case that created the Requirements `created`.
pub fn read_list(created: &[String]) -> (Vec<String>, usize) {
    let mut reads: Vec<String> = READS
        .iter()
        .filter(|path| {
            path.split('?')
                .next()
                .is_some_and(|path| path.matches('/').count() == 1)
        })
        .map(|path| (*path).to_owned())
        .collect();
    let lists = reads.len();
    let mut requirements = vec![FIRST_REQUIREMENT.to_owned(), DETAIL_REQUIREMENT.to_owned()];
    requirements.extend(created.iter().cloned());
    for id in requirements {
        reads.push(format!("/requirements/{id}"));
        for query in ["neighbors", "trace", "impact"] {
            reads.push(format!("/requirements/{id}?query={query}"));
        }
        reads.push(format!("/requirements/{id}/document"));
    }
    reads.push(format!("/rules/{FIRST_RULE}?query=neighbors"));
    reads.push(format!("/rules/{FIRST_RULE}/evidence"));
    reads.push(format!("/topics/{TOPIC}?query=neighbors"));
    (reads, lists)
}

/// Checks L6 on `repo` for a case that created `created`, after `context`.
pub fn check_reads(repo: &Repository, created: &[String], context: &str) -> Result<(), String> {
    let (paths, lists) = read_list(created);
    let reads = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let caught_up = Reads(repo.dir.path().display().to_string());
        let caught_up = now(read_paths(&caught_up, repo, &paths));
        let copy = copy_without_projection(repo);
        let rebuilt = Reads(copy.dir.path().display().to_string());
        let rebuilt = now(read_paths(&rebuilt, &copy, &paths));
        assert_same(&caught_up, &rebuilt, context);
        rebuilt
    }));
    let rebuilt = reads.map_err(|payload| report::panic_text(&*payload))?;
    let missing: Vec<&Value> = rebuilt[lists..]
        .iter()
        .filter(|read| read["answer"]["status"] != 200)
        .collect();
    if let Some(read) = missing.first() {
        return Err(format!("{context}: a record read found no record: {read}"));
    }
    Ok(())
}
