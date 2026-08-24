use super::super::git::{ChangeKind, ChangedFile, LineSpan};
use crate::wiki::links::{parse_code_ref, parse_code_refs, CodeRef, LineRange};
use camino::Utf8Path;
use provenance_core::coverage::{
    BindingResult, EvidenceDiffSite, EvidenceDiffState, EvidenceSiteKind,
};
use provenance_store::cache::GraphEvidenceReference;

pub(super) fn sites(
    reference: &GraphEvidenceReference,
    bindings: &[BindingResult],
    changes: &[ChangedFile],
) -> Vec<EvidenceDiffSite> {
    if reference.section.as_deref().is_some_and(|section| {
        bindings.iter().any(|binding| {
            binding.rule_id == reference.subject_id
                && binding.item_name.as_deref() == Some(section)
                && (same_path(&binding.file_path, &reference.document)
                    || binding
                        .original_file_path
                        .as_deref()
                        .is_some_and(|path| same_path(path, &reference.document)))
        })
    }) {
        // The scanner site is the same physical evidence with a durable
        // anchor. Keep one row rather than count source_document and #[rule].
        return Vec::new();
    }
    graph_code_refs(reference)
        .iter()
        .map(|code_ref| path_site(reference, code_ref, changes))
        .collect()
}

fn graph_code_refs(reference: &GraphEvidenceReference) -> Vec<CodeRef> {
    let document = reference
        .document
        .strip_prefix("file://")
        .unwrap_or(&reference.document);
    reference
        .section
        .as_deref()
        .and_then(|section| parse_code_ref(&format!("{document}:{section}")))
        .filter(|code_ref| !code_ref.lines.is_empty())
        .map_or_else(|| parse_code_refs(document), |code_ref| vec![code_ref])
}

fn path_site(
    reference: &GraphEvidenceReference,
    code_ref: &CodeRef,
    changes: &[ChangedFile],
) -> EvidenceDiffSite {
    let path = Utf8Path::new(code_ref.path.strip_prefix("./").unwrap_or(&code_ref.path));
    let change = changes
        .iter()
        .find(|change| change.old_path == path || change.new_path == path);
    let (file_path, state, original_file_path) = match change {
        Some(change) if change.kind == ChangeKind::Deleted => {
            (path.to_path_buf(), EvidenceDiffState::Gone, None)
        }
        Some(change) if change.kind == ChangeKind::Renamed => {
            let state = if !change.new_lines.is_empty()
                && reference_lines_changed(&code_ref.lines, &change.new_lines)
            {
                EvidenceDiffState::Touched
            } else {
                EvidenceDiffState::Moved
            };
            (
                change.new_path.clone(),
                state,
                Some(change.old_path.clone()),
            )
        }
        Some(change) if reference_lines_changed(&code_ref.lines, &change.new_lines) => {
            (path.to_path_buf(), EvidenceDiffState::Touched, None)
        }
        Some(_) | None => (path.to_path_buf(), EvidenceDiffState::Untouched, None),
    };
    let line = code_ref.lines.first().map(|range| range.start as usize);
    let end_line = code_ref
        .lines
        .first()
        .map(|range| range.end.unwrap_or(range.start) as usize);
    EvidenceDiffSite {
        kind: EvidenceSiteKind::SourceReference,
        subject_id: reference.subject_id.clone(),
        file_path,
        line,
        end_line,
        state,
        original_file_path,
        original_line: None,
    }
}

fn reference_lines_changed(ranges: &[LineRange], changes: &[LineSpan]) -> bool {
    ranges.is_empty()
        || ranges.iter().any(|range| {
            changes.iter().any(|change| {
                change.intersects(
                    range.start as usize,
                    range.end.unwrap_or(range.start) as usize,
                )
            })
        })
}

fn same_path(path: &Utf8Path, document: &str) -> bool {
    path == Utf8Path::new(document.strip_prefix("./").unwrap_or(document))
}
