use super::annotate::{is_test_name, tokens, trim_token};
use super::remote::{blob_url, parse_git_remote, GitRemote};
use provenance_core::coverage::CoverageScan;
use provenance_macros::rule;
use provenance_store::code_refs::{parse_code_ref, CodeRef};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod reference_links;

/// An evidence reference resolved for display: the original text plus a
/// link target when one could be derived. `href` is `None` for prose
/// references so the renderer shows them as plain text, never a dead link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceRef {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<EvidenceSnippet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceSnippet {
    pub label: String,
    pub content: String,
}

/// A linkable span inside free text (byte offsets into the original text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InlineRef {
    pub start: usize,
    pub end: usize,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<EvidenceSnippet>,
}

#[derive(Debug, Clone, Default)]
struct ScanContext {
    commit: Option<String>,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathResolution {
    Exists,
    Missing,
    Unavailable,
    Unknown,
}

/// Resolves evidence references against an optional git remote.
#[derive(Debug, Clone, Default)]
pub struct LinkResolver {
    remote: Option<GitRemote>,
    scan: Option<ScanContext>,
    repository: Option<PathBuf>,
}

impl LinkResolver {
    pub fn new(remote_url: Option<&str>) -> Self {
        Self {
            remote: remote_url.and_then(parse_git_remote),
            scan: None,
            repository: None,
        }
    }

    pub fn with_repository(&self, repository: &Path) -> Self {
        Self {
            remote: self.remote.clone(),
            scan: self.scan.clone(),
            repository: Some(repository.to_path_buf()),
        }
    }

    pub fn with_coverage(&self, scan: &CoverageScan) -> Self {
        let mut files: BTreeMap<String, String> = scan
            .scanned_files
            .iter()
            .map(|file| {
                (
                    normalized_path(file.file_path.as_str()).to_string(),
                    file.content.clone(),
                )
            })
            .collect();
        for binding in &scan.report.bindings {
            files
                .entry(normalized_path(binding.file_path.as_str()).to_string())
                .or_default();
        }
        Self {
            remote: self.remote.clone(),
            scan: Some(ScanContext {
                commit: scan.commit.clone(),
                files,
            }),
            repository: self.repository.clone(),
        }
    }

    /// Resolves a reference without promising a mutable revision.
    pub fn resolve(&self, reference: &str) -> EvidenceRef {
        self.resolve_at(reference, None)
    }

    /// Resolves a reference pinned to a commit when one is known.
    ///
    /// A recorded reference is shown as a clickable link only when the link
    /// would actually work: either the reference is already a web address,
    /// which is used as it stands, or it reads as a file path that a known
    /// code host can turn into a real blob URL at a known immutable revision.
    /// With no known remote or commit, a path would either resolve against the
    /// wiki page's route or require a mutable `HEAD` link, so it stays plain
    /// text: text the reader can search for beats a link that lies.
    ///
    /// A file path is also checked against the tree it cites: the scanned
    /// file list when the scan covers the requested commit, otherwise
    /// `git ls-tree` at the pinned commit (`HEAD` when unpinned). A path the
    /// tree does not contain stays plain text with a short note saying why.
    /// `file://` URLs never link at all — a reader's browser cannot reach
    /// them — so they carry the same kind of note instead.
    ///
    /// "Reads as a file path" is decided in one place, `parse_code_ref`: a
    /// directory separator, or a bare name carrying a line group. A bare
    /// dotted token with neither (`e.g.`, `Fig.`, `v1.2`, and equally
    /// `payroll.rs`) is prose and stays plain text, even when it is the
    /// whole field. The resolver has nothing to tell the abbreviation from
    /// the file name; the writer who wants the link adds a directory or a
    /// line number.
    #[rule("rule_wiki_reference_links")]
    pub fn resolve_at(&self, reference: &str, commit: Option<&str>) -> EvidenceRef {
        let label = reference.trim().to_string();
        if label.starts_with("http://") || label.starts_with("https://") {
            let href = Some(label.clone());
            return EvidenceRef {
                label,
                href,
                note: None,
                snippet: None,
            };
        }
        if label.starts_with("file://") {
            return EvidenceRef {
                label,
                href: None,
                note: Some("local file URL is unavailable to wiki readers".to_string()),
                snippet: None,
            };
        }
        if let Some(code_ref) = parse_code_ref(&label) {
            return self.resolve_code_ref(label, &code_ref, commit);
        }
        EvidenceRef {
            label,
            href: None,
            note: None,
            snippet: None,
        }
    }

    /// Resolves a document/section pair, as stored on rules
    /// (`source_document` + `source_section`). A section that parses as line
    /// ranges is folded into the link anchor; a prose section stays visible
    /// in the label while the document alone is linked.
    pub fn resolve_document(
        &self,
        document: &str,
        section: Option<&str>,
        commit: Option<&str>,
    ) -> EvidenceRef {
        let Some(section) = section else {
            return self.resolve_at(document, commit);
        };
        let combined = format!("{document}:{section}");
        if let Some(code_ref) = parse_code_ref(&combined) {
            if !code_ref.lines.is_empty() {
                return self.resolve_code_ref(combined, &code_ref, commit);
            }
        }
        let mut evidence = self.resolve_at(document, commit);
        evidence.label = format!("{document} ({section})");
        evidence
    }

    /// Finds linkable spans in free text: code references, and test-case
    /// names which link to the file of the nearest code reference in the
    /// same text. Bare dotted words without a separator or line group (for
    /// example `e.g.`) are left alone to avoid false positives; that
    /// judgement lives in `parse_code_ref`, which every surface shares.
    pub fn annotate(&self, text: &str) -> Vec<InlineRef> {
        let mut file_refs: Vec<(InlineRef, Option<String>)> = Vec::new();
        let mut test_names: Vec<InlineRef> = Vec::new();
        for (token_start, token) in tokens(text) {
            let (offset, trimmed) = trim_token(token);
            if trimmed.is_empty() {
                continue;
            }
            let start = token_start + offset;
            let end = start + trimmed.len();
            if let Some(code_ref) = parse_code_ref(trimmed) {
                if !self.scan_contains(&code_ref.path) {
                    continue;
                }
                // Only paths present in the scan reach this point. A remote
                // and commit may add an anchor; scanned content can still
                // provide a local snippet when no safe anchor exists.
                let file_href = self.href_for(
                    &CodeRef {
                        path: code_ref.path.clone(),
                        lines: Vec::new(),
                    },
                    None,
                );
                let inline = InlineRef {
                    start,
                    end,
                    label: trimmed.to_string(),
                    href: self.href_for(&code_ref, None),
                    snippet: self.snippet_for(&code_ref, None),
                };
                file_refs.push((inline, file_href));
                continue;
            }
            if is_test_name(trimmed) {
                test_names.push(InlineRef {
                    start,
                    end,
                    label: trimmed.to_string(),
                    href: None,
                    snippet: None,
                });
            }
        }
        let mut refs: Vec<InlineRef> = Vec::new();
        for mut test_name in test_names {
            let nearest = file_refs
                .iter()
                .rev()
                .find(|(inline, _)| inline.start < test_name.start)
                .or_else(|| file_refs.first());
            if let Some((_, Some(file_href))) = nearest {
                test_name.href = Some(file_href.clone());
                refs.push(test_name);
            }
        }
        refs.extend(file_refs.into_iter().map(|(inline, _)| inline));
        refs.sort_by_key(|inline| inline.start);
        refs
    }

    /// Builds a commit-pinned blob URL for `code_ref` on the known remote.
    /// Returns `None` without both a remote and an immutable revision.
    fn href_for(&self, code_ref: &CodeRef, commit: Option<&str>) -> Option<String> {
        if self.scanned_file_excludes_location(code_ref, commit) {
            return None;
        }
        let reference = commit
            .map(str::to_string)
            .or_else(|| self.scan.as_ref().and_then(|scan| scan.commit.clone()))
            .filter(|reference| !reference.eq_ignore_ascii_case("HEAD"))?;
        self.remote
            .as_ref()
            .map(|remote| blob_url(remote, &reference, code_ref))
    }

    fn scanned_file_excludes_location(&self, code_ref: &CodeRef, commit: Option<&str>) -> bool {
        if code_ref.lines.is_empty() {
            return false;
        }
        let Some(scan) = self.scan.as_ref() else {
            return false;
        };
        if commit.is_some_and(|commit| {
            !scan
                .commit
                .as_deref()
                .is_some_and(|scan_commit| commit_matches(scan_commit, commit))
        }) {
            return false;
        }
        let Some(content) = scan.files.get(normalized_path(&code_ref.path)) else {
            return false;
        };
        // Binding-registered paths carry no content: the scan attests the
        // file exists at the commit but says nothing about its length.
        if content.is_empty() {
            return false;
        }
        let line_count = content.lines().count();
        code_ref
            .lines
            .iter()
            .any(|range| range.start == 0 || range.end.unwrap_or(range.start) as usize > line_count)
    }

    fn scan_contains(&self, path: &str) -> bool {
        self.scan
            .as_ref()
            .is_some_and(|scan| scan.files.contains_key(normalized_path(path)))
    }

    fn resolve_code_ref(
        &self,
        label: String,
        code_ref: &CodeRef,
        commit: Option<&str>,
    ) -> EvidenceRef {
        let path_resolution = self.path_resolution(&code_ref.path, commit);
        let linkable = matches!(
            path_resolution,
            PathResolution::Exists | PathResolution::Unknown
        );
        EvidenceRef {
            label,
            href: linkable.then(|| self.href_for(code_ref, commit)).flatten(),
            note: match path_resolution {
                PathResolution::Missing => Some(self.missing_path_note(commit).to_string()),
                PathResolution::Unavailable => Some(self.unavailable_tree_note(commit).to_string()),
                PathResolution::Exists | PathResolution::Unknown => None,
            },
            snippet: linkable
                .then(|| self.snippet_for(code_ref, commit))
                .flatten(),
        }
    }

    fn missing_path_note(&self, commit: Option<&str>) -> &'static str {
        if self.tree_is_pinned(commit) {
            "path not found in the pinned tree"
        } else if self.scan.is_some() {
            "path not found in the scanned files"
        } else {
            "path not found in the repository tree"
        }
    }

    fn unavailable_tree_note(&self, commit: Option<&str>) -> &'static str {
        if self.tree_is_pinned(commit) {
            "pinned tree unavailable"
        } else {
            "repository tree unavailable"
        }
    }

    fn tree_is_pinned(&self, commit: Option<&str>) -> bool {
        commit.is_some() || self.scan.as_ref().is_some_and(|scan| scan.commit.is_some())
    }

    fn path_resolution(&self, path: &str, commit: Option<&str>) -> PathResolution {
        let mut absent_from_matching_scan = false;
        if let Some(scan) = &self.scan {
            let same_tree = commit.is_none_or(|commit| {
                scan.commit
                    .as_deref()
                    .is_some_and(|scan_commit| commit_matches(scan_commit, commit))
            });
            if same_tree {
                if scan.files.contains_key(normalized_path(path)) {
                    return PathResolution::Exists;
                }
                absent_from_matching_scan = true;
            }
        }
        let Some(repository) = self.repository.as_ref() else {
            return if absent_from_matching_scan {
                PathResolution::Missing
            } else {
                PathResolution::Unknown
            };
        };
        let revision = commit
            .or_else(|| self.scan.as_ref().and_then(|scan| scan.commit.as_deref()))
            .unwrap_or("HEAD");
        let Ok(output) = std::process::Command::new("git")
            .current_dir(repository)
            .args(["ls-tree", "-r", "-z", "--name-only", revision, "--", path])
            .output()
        else {
            return PathResolution::Unavailable;
        };
        if !output.status.success() {
            return PathResolution::Unavailable;
        }
        if output
            .stdout
            .split(|byte| *byte == 0)
            .any(|candidate| candidate == normalized_path(path).as_bytes())
        {
            PathResolution::Exists
        } else {
            PathResolution::Missing
        }
    }

    fn snippet_for(&self, code_ref: &CodeRef, commit: Option<&str>) -> Option<EvidenceSnippet> {
        if code_ref.lines.is_empty() {
            return None;
        }
        let scan = self.scan.as_ref()?;
        if commit.is_some_and(|commit| {
            !scan
                .commit
                .as_deref()
                .is_some_and(|scan_commit| commit_matches(scan_commit, commit))
        }) {
            return None;
        }
        let content = scan.files.get(normalized_path(&code_ref.path))?;
        let lines = content.lines().collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut displayed_ranges = Vec::new();
        for range in &code_ref.lines {
            let start = range.start as usize;
            let requested_end = range.end.unwrap_or(range.start) as usize;
            if start == 0 || start > lines.len() {
                continue;
            }
            let displayed_end = requested_end.min(lines.len()).min(start.saturating_add(11));
            let mut content = lines[start - 1..displayed_end].join("\n");
            if displayed_end < requested_end {
                content.push_str("\n…");
            }
            selected.push(content);
            displayed_ranges.push(if displayed_end == start {
                start.to_string()
            } else {
                format!("{start}-{displayed_end}")
            });
        }
        let requested_ranges = format_ranges(code_ref);
        let displayed_ranges = displayed_ranges.join(", ");
        (!selected.is_empty()).then(|| EvidenceSnippet {
            label: if displayed_ranges == requested_ranges {
                format!("{}:{requested_ranges}", code_ref.path)
            } else {
                format!(
                    "{}:{displayed_ranges} (requested {requested_ranges})",
                    code_ref.path
                )
            },
            content: selected.join("\n…\n"),
        })
    }
}

fn format_ranges(code_ref: &CodeRef) -> String {
    code_ref
        .lines
        .iter()
        .map(|range| {
            range.end.map_or_else(
                || range.start.to_string(),
                |end| format!("{}-{end}", range.start),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn commit_matches(scan_commit: &str, requested_commit: &str) -> bool {
    scan_commit
        .get(..requested_commit.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(requested_commit))
}

fn normalized_path(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

#[cfg(test)]
mod tests;
