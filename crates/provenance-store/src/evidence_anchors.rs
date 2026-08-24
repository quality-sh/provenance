use provenance_core::coverage::{
    AnchorState, AnnotationResult, BindingResult, CoverageScan, EvidenceAnchor, ValidationWarning,
};
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::handlers) fn reconcile(
    current: &mut CoverageScan,
    baseline: &CoverageScan,
    repo: &camino::Utf8Path,
    scan_path: &camino::Utf8Path,
    validate_rules: bool,
) {
    let paths = ScanPaths::new(repo, scan_path);
    let (annotations, mut ambiguity_warnings) = reconcile_sites(
        &baseline.report.annotations,
        &current.report.annotations,
        &paths,
    );
    current.report.annotations = annotations;
    let (bindings, binding_warnings) =
        reconcile_sites(&baseline.report.bindings, &current.report.bindings, &paths);
    current.report.bindings = bindings;
    ambiguity_warnings.extend(binding_warnings);
    current.report.warnings.extend(ambiguity_warnings);
    current.report.total_annotations = current
        .report
        .annotations
        .iter()
        .filter(|site| site.anchor_state != AnchorState::Gone)
        .count();
    if validate_rules {
        current.report.warnings.extend(gone_site_warnings(current));
    }
}

struct ScanPaths<'a> {
    repo: &'a camino::Utf8Path,
    canonical_repo: Option<camino::Utf8PathBuf>,
    scan_path: &'a camino::Utf8Path,
}

impl<'a> ScanPaths<'a> {
    fn new(repo: &'a camino::Utf8Path, scan_path: &'a camino::Utf8Path) -> Self {
        let canonical_repo = std::fs::canonicalize(repo)
            .ok()
            .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok());
        Self {
            repo,
            canonical_repo,
            scan_path,
        }
    }

    fn relative<'p>(&self, path: &'p camino::Utf8Path) -> &'p camino::Utf8Path {
        if let Ok(relative) = path.strip_prefix(self.repo) {
            return relative;
        }
        if let Some(relative) = self
            .canonical_repo
            .as_deref()
            .and_then(|repo| path.strip_prefix(repo).ok())
        {
            return relative;
        }
        path.strip_prefix(".").unwrap_or(path)
    }

    fn contains(&self, path: &camino::Utf8Path) -> bool {
        self.normalized(path)
            .starts_with(self.normalized(self.scan_path))
    }

    fn same_file(&self, left: &camino::Utf8Path, right: &camino::Utf8Path) -> bool {
        self.normalized(left) == self.normalized(right)
    }

    fn normalized(&self, path: &camino::Utf8Path) -> camino::Utf8PathBuf {
        let canonical = std::fs::canonicalize(path)
            .ok()
            .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok());
        lexical_normalize(self.relative(canonical.as_deref().unwrap_or(path)))
    }
}

fn lexical_normalize(path: &camino::Utf8Path) -> camino::Utf8PathBuf {
    let mut normalized = camino::Utf8PathBuf::new();
    for component in path.components() {
        match component {
            camino::Utf8Component::CurDir => {}
            camino::Utf8Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            _ => normalized.push(component.as_str()),
        }
    }
    normalized
}

/// One reconcilable scan result: an annotation or a binding. Both carry the
/// same anchor bookkeeping, and reconciliation reads nothing else.
trait AnchoredSite: Clone {
    const KIND: &'static str;
    fn rule_id(&self) -> &str;
    fn file_path(&self) -> &camino::Utf8Path;
    fn line(&self) -> usize;
    fn anchor(&self) -> Option<&EvidenceAnchor>;
    fn mark(
        &mut self,
        state: AnchorState,
        original_line: Option<usize>,
        original_file_path: Option<camino::Utf8PathBuf>,
    );
}

impl AnchoredSite for AnnotationResult {
    const KIND: &'static str = "annotation";

    fn rule_id(&self) -> &str {
        &self.rule_id
    }

    fn file_path(&self) -> &camino::Utf8Path {
        &self.file_path
    }

    fn line(&self) -> usize {
        self.line
    }

    fn anchor(&self) -> Option<&EvidenceAnchor> {
        self.anchor.as_ref()
    }

    fn mark(
        &mut self,
        state: AnchorState,
        original_line: Option<usize>,
        original_file_path: Option<camino::Utf8PathBuf>,
    ) {
        self.anchor_state = state;
        self.original_line = original_line;
        self.original_file_path = original_file_path;
    }
}

impl AnchoredSite for BindingResult {
    const KIND: &'static str = "binding";

    fn rule_id(&self) -> &str {
        &self.rule_id
    }

    fn file_path(&self) -> &camino::Utf8Path {
        &self.file_path
    }

    fn line(&self) -> usize {
        self.line
    }

    fn anchor(&self) -> Option<&EvidenceAnchor> {
        self.anchor.as_ref()
    }

    fn mark(
        &mut self,
        state: AnchorState,
        original_line: Option<usize>,
        original_file_path: Option<camino::Utf8PathBuf>,
    ) {
        self.anchor_state = state;
        self.original_line = original_line;
        self.original_file_path = original_file_path;
    }
}

/// Resolve every baseline anchor against the current scan.
///
/// Four passes, strongest claim first. A baseline site is pinned to an exact
/// file and line, then moved within its file when it is the only unresolved
/// holder of its anchor there, then relocated across files when exactly one
/// baseline site and one current site share the anchor. What that leaves is
/// either gone (no current site holds the anchor) or ambiguous, and ambiguity
/// is said out loud in a warning instead of guessed at or absorbed.
///
/// Current sites whose anchor no baseline site holds are `new`: the scan has
/// nothing to compare them against and claims nothing.
fn reconcile_sites<S: AnchoredSite>(
    baseline: &[S],
    current: &[S],
    paths: &ScanPaths<'_>,
) -> (Vec<S>, Vec<ValidationWarning>) {
    let tracked = baseline
        .iter()
        .enumerate()
        .filter(|(_, site)| site.anchor().is_some() && paths.contains(site.file_path()))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut reconciliation = Reconciliation {
        baseline,
        current,
        tracked,
        used: BTreeSet::new(),
        refused: BTreeSet::new(),
        by_previous: vec![None; baseline.len()],
        warnings: Vec::new(),
    };
    reconciliation.pin_exact(paths);
    reconciliation.move_within_file(paths);
    reconciliation.resolve_anchor_groups(paths);
    reconciliation.finish()
}

struct Reconciliation<'a, S: AnchoredSite> {
    baseline: &'a [S],
    current: &'a [S],
    /// Baseline indices carrying an anchor inside the scanned path.
    tracked: Vec<usize>,
    /// Current indices claimed by a baseline site or an ambiguity group.
    used: BTreeSet<usize>,
    /// Current indices the scan refused to pair: they hold a contested
    /// anchor and stay at their coordinates as unchanged.
    refused: BTreeSet<usize>,
    /// The resolution of each baseline site, in baseline order.
    by_previous: Vec<Option<S>>,
    warnings: Vec<ValidationWarning>,
}

impl<S: AnchoredSite> Reconciliation<'_, S> {
    /// Pin by exact file and line first, so a surviving duplicate keeps its
    /// identity and only the genuinely missing instances fall through.
    fn pin_exact(&mut self, paths: &ScanPaths<'_>) {
        for position in 0..self.tracked.len() {
            let previous_index = self.tracked[position];
            let previous = &self.baseline[previous_index];
            if let Some((current_index, found)) =
                self.current.iter().enumerate().find(|(index, site)| {
                    !self.used.contains(index)
                        && site.line() == previous.line()
                        && sites_match(previous, *site, paths)
                })
            {
                self.used.insert(current_index);
                self.by_previous[previous_index] =
                    Some(resolved(found, AnchorState::Unchanged, None, None));
            }
        }
    }

    /// A within-file move: one unresolved baseline holder of the anchor, one
    /// unclaimed current holder in the same file.
    fn move_within_file(&mut self, paths: &ScanPaths<'_>) {
        for position in 0..self.tracked.len() {
            let previous_index = self.tracked[position];
            if self.by_previous[previous_index].is_some() {
                continue;
            }
            let previous = &self.baseline[previous_index];
            let candidates = self
                .current
                .iter()
                .enumerate()
                .filter(|(index, site)| {
                    !self.used.contains(index) && sites_match(previous, *site, paths)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let unresolved_duplicates = self
                .tracked
                .iter()
                .filter(|&&index| {
                    self.by_previous[index].is_none()
                        && sites_match(&self.baseline[index], previous, paths)
                })
                .count();
            if candidates.len() == 1 && unresolved_duplicates == 1 {
                self.used.insert(candidates[0]);
                self.by_previous[previous_index] = Some(resolved(
                    &self.current[candidates[0]],
                    AnchorState::Moved,
                    Some(previous.line()),
                    None,
                ));
            }
        }
    }

    /// What is left resolves per anchor, not per site, so a cross-file move
    /// is found before anything is declared missing and duplicate loss cannot
    /// hide behind a surviving twin.
    fn resolve_anchor_groups(&mut self, paths: &ScanPaths<'_>) {
        let mut groups: BTreeMap<(String, Option<String>, String), Vec<usize>> = BTreeMap::new();
        for &previous_index in &self.tracked {
            if self.by_previous[previous_index].is_some() {
                continue;
            }
            let previous = &self.baseline[previous_index];
            let anchor = previous
                .anchor()
                .expect("tracked baseline sites carry anchors");
            groups
                .entry((
                    previous.rule_id().to_string(),
                    anchor.symbol.clone(),
                    anchor.content_hash.clone(),
                ))
                .or_default()
                .push(previous_index);
        }
        for members in groups.values() {
            self.resolve_group(members, paths);
        }
    }

    fn resolve_group(&mut self, members: &[usize], paths: &ScanPaths<'_>) {
        let previous = &self.baseline[members[0]];
        let candidates = self
            .current
            .iter()
            .enumerate()
            .filter(|(index, site)| {
                !self.used.contains(index)
                    && site.rule_id() == previous.rule_id()
                    && site.anchor() == previous.anchor()
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            for &previous_index in members {
                self.by_previous[previous_index] = Some(resolved(
                    &self.baseline[previous_index],
                    AnchorState::Gone,
                    None,
                    None,
                ));
            }
            return;
        }
        if members.len() == 1 && candidates.len() == 1 {
            let found = &self.current[candidates[0]];
            let original_file = (!paths.same_file(previous.file_path(), found.file_path()))
                .then(|| previous.file_path().to_path_buf());
            self.used.insert(candidates[0]);
            self.by_previous[members[0]] = Some(resolved(
                found,
                AnchorState::Moved,
                Some(previous.line()),
                original_file,
            ));
            return;
        }
        // Identical duplicates shuffled within one file, none lost: the
        // sites are interchangeable, so nothing worth a warning happened.
        let same_file_shuffle = members.len() == candidates.len()
            && members.iter().all(|&index| {
                paths.same_file(self.baseline[index].file_path(), previous.file_path())
            })
            && candidates.iter().all(|&index| {
                paths.same_file(self.current[index].file_path(), previous.file_path())
            });
        for &index in &candidates {
            self.used.insert(index);
            self.refused.insert(index);
        }
        if !same_file_shuffle {
            self.warnings.push(ambiguity_warning(
                self.baseline,
                self.current,
                members,
                &candidates,
            ));
        }
    }

    fn finish(self) -> (Vec<S>, Vec<ValidationWarning>) {
        let mut resolved_sites = self.by_previous.into_iter().flatten().collect::<Vec<_>>();
        for (index, site) in self.current.iter().enumerate() {
            if self.refused.contains(&index) {
                resolved_sites.push(resolved(site, AnchorState::Unchanged, None, None));
            } else if !self.used.contains(&index) {
                resolved_sites.push(resolved(site, AnchorState::New, None, None));
            }
        }
        (resolved_sites, self.warnings)
    }
}

fn sites_match<S: AnchoredSite>(left: &S, right: &S, paths: &ScanPaths<'_>) -> bool {
    paths.same_file(left.file_path(), right.file_path())
        && left.rule_id() == right.rule_id()
        && left.anchor() == right.anchor()
}

fn resolved<S: AnchoredSite>(
    site: &S,
    state: AnchorState,
    original_line: Option<usize>,
    original_file_path: Option<camino::Utf8PathBuf>,
) -> S {
    let mut resolved = site.clone();
    resolved.mark(state, original_line, original_file_path);
    resolved
}

/// Several sites hold one anchor and the pairing is not one-to-one. The scan
/// names every party instead of picking a pairing, and when the group shrank
/// it says by how much, because a silent survivor would otherwise absorb the
/// loss.
fn ambiguity_warning<S: AnchoredSite>(
    baseline: &[S],
    current: &[S],
    members: &[usize],
    candidates: &[usize],
) -> ValidationWarning {
    let locations = |indices: &[usize], sites: &[S]| {
        indices
            .iter()
            .map(|&index| format!("{}:{}", sites[index].file_path(), sites[index].line()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let loss = if members.len() > candidates.len() {
        format!(
            "; the group lost {} instance(s)",
            members.len() - candidates.len()
        )
    } else {
        String::new()
    };
    let message = format!(
        "{} anchored {} site(s) ({}) match {} current site(s) ({}); the scan cannot pair them{loss}",
        members.len(),
        S::KIND,
        locations(members, baseline),
        candidates.len(),
        locations(candidates, current),
    );
    let first = &baseline[members[0]];
    ValidationWarning {
        rule_id: first.rule_id().to_string(),
        file_path: Some(first.file_path().to_path_buf()),
        line: Some(first.line()),
        message,
    }
}

fn gone_site_warnings(report: &CoverageScan) -> Vec<ValidationWarning> {
    report
        .annotations
        .iter()
        .filter(|site| site.anchor_state == AnchorState::Gone)
        .map(|site| warning(&site.rule_id, &site.file_path, site.line, "annotation"))
        .chain(
            report
                .bindings
                .iter()
                .filter(|site| site.anchor_state == AnchorState::Gone)
                .map(|site| warning(&site.rule_id, &site.file_path, site.line, "binding")),
        )
        .collect()
}

fn warning(
    rule_id: &str,
    file_path: &camino::Utf8Path,
    line: usize,
    kind: &str,
) -> ValidationWarning {
    ValidationWarning {
        rule_id: rule_id.to_string(),
        file_path: Some(file_path.to_path_buf()),
        line: Some(line),
        message: format!("anchored {kind} site is gone"),
    }
}
