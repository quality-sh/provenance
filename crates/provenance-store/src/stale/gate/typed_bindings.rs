use super::super::git::{ChangeKind, ChangedFile};
use camino::Utf8Path;
use provenance_core::{
    coverage::{CoverageScan, EvidenceDiffSite, EvidenceDiffState, EvidenceSiteKind},
    ImplementationBinding,
};
use provenance_store::cache::GraphEvidence;

pub(super) fn sites(
    scanned: &[EvidenceDiffSite],
    scan: &CoverageScan,
    graph: &GraphEvidence,
    changes: &[ChangedFile],
) -> Vec<EvidenceDiffSite> {
    let implementations = graph
        .implementation_bindings
        .iter()
        .filter(|binding| !scanned_implementation_matches(scan, binding))
        .map(|binding| {
            project(
                EvidenceSiteKind::RuleBinding,
                binding.rule_id.as_str(),
                &binding.file,
                changes,
            )
        });
    let verifications = graph.verification_bindings.iter().filter_map(|binding| {
        project_unless_scanned(
            scanned,
            EvidenceSiteKind::Verification,
            binding.rule_id.as_str(),
            &binding.file,
            changes,
        )
    });
    implementations.chain(verifications).collect()
}

fn scanned_implementation_matches(scan: &CoverageScan, binding: &ImplementationBinding) -> bool {
    scan.annotations.iter().any(|site| {
        site.verification.is_none()
            && site.rule_id == binding.rule_id.as_str()
            && site.function_name.as_deref() == Some(binding.symbol.as_str())
            && matches_file(
                &site.file_path,
                site.original_file_path.as_deref(),
                &binding.file,
            )
    }) || scan.bindings.iter().any(|site| {
        site.verification.is_none()
            && site.rule_id == binding.rule_id.as_str()
            && site.item_name.as_deref() == Some(binding.symbol.as_str())
            && matches_file(
                &site.file_path,
                site.original_file_path.as_deref(),
                &binding.file,
            )
    })
}

fn matches_file(current: &Utf8Path, original: Option<&Utf8Path>, expected: &Utf8Path) -> bool {
    current == expected || original == Some(expected)
}

fn project_unless_scanned(
    scanned: &[EvidenceDiffSite],
    kind: EvidenceSiteKind,
    rule_id: &str,
    file: &Utf8Path,
    changes: &[ChangedFile],
) -> Option<EvidenceDiffSite> {
    if scanned.iter().any(|site| {
        site.kind == kind
            && site.subject_id == rule_id
            && (site.file_path == file || site.original_file_path.as_deref() == Some(file))
    }) {
        return None;
    }
    Some(project(kind, rule_id, file, changes))
}

fn project(
    kind: EvidenceSiteKind,
    rule_id: &str,
    file: &Utf8Path,
    changes: &[ChangedFile],
) -> EvidenceDiffSite {
    let change = changes
        .iter()
        .find(|change| change.old_path == file || change.new_path == file);
    let (file_path, state, original_file_path) = match change {
        Some(change) if change.kind == ChangeKind::Deleted => {
            (file.to_path_buf(), EvidenceDiffState::Gone, None)
        }
        Some(change) if change.kind == ChangeKind::Renamed => (
            change.new_path.clone(),
            EvidenceDiffState::Moved,
            Some(change.old_path.clone()),
        ),
        Some(_) => (file.to_path_buf(), EvidenceDiffState::Touched, None),
        None => (file.to_path_buf(), EvidenceDiffState::Untouched, None),
    };
    EvidenceDiffSite {
        kind,
        subject_id: rule_id.to_string(),
        file_path,
        line: None,
        end_line: None,
        state,
        original_file_path,
        original_line: None,
    }
}
