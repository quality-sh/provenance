use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::protocol::{AffectedRule, ImplementationSite, VerificationSite};
use provenance_core::{ImplementationBinding, StableId, VerificationBinding};
use provenance_scanner::{source_sites, SourceSiteRole};

/// Everything a reader needs to place one Rule in the code.
///
/// Scanner sites and canonical bindings answer the same question from two
/// directions, so both surfaces read them together and in one order.
pub(super) struct Evidence<'a> {
    pub scans: &'a [provenance_scanner::FileScan],
    pub verifications: &'a [VerificationBinding],
    pub implementations: &'a [ImplementationBinding],
}

impl Evidence<'_> {
    pub(super) fn affected_rule(&self, repo: &Utf8Path, id: StableId) -> AffectedRule {
        AffectedRule {
            implementations: self.implementations(repo, &id),
            verifications: self.verifications(repo, &id),
            id,
        }
    }

    fn implementations(&self, repo: &Utf8Path, id: &StableId) -> Vec<ImplementationSite> {
        let mut sites = source_sites(self.scans)
            .filter(|site| site.rule_id() == id.as_str())
            .filter(|site| site.role() == SourceSiteRole::Implementation)
            .map(|site| ImplementationSite {
                file: relative(repo, site.file_path()),
                line: Some(site.line()),
                symbol: None,
            })
            .chain(
                self.implementations
                    .iter()
                    .filter(|binding| !binding.retired && binding.rule_id == *id)
                    .map(|binding| ImplementationSite {
                        file: binding.file.clone(),
                        line: None,
                        symbol: Some(binding.symbol.clone()),
                    }),
            )
            .collect::<Vec<_>>();
        sites.sort();
        sites.dedup();
        sites
    }

    fn verifications(&self, repo: &Utf8Path, id: &StableId) -> Vec<VerificationSite> {
        let mut sites = source_sites(self.scans)
            .filter(|site| site.rule_id() == id.as_str())
            .filter_map(|site| {
                site.verification().map(|method| VerificationSite {
                    key: None,
                    method: method.to_string(),
                    declared_by: None,
                    file: relative(repo, site.file_path()),
                    line: Some(site.line()),
                    symbol: None,
                })
            })
            .chain(
                self.verifications
                    .iter()
                    .filter(|binding| binding.rule_id == *id)
                    .map(typed_verification),
            )
            .collect::<Vec<_>>();
        sites.sort();
        sites.dedup();
        sites
    }
}

fn typed_verification(binding: &VerificationBinding) -> VerificationSite {
    VerificationSite {
        key: Some(binding.key.clone()),
        method: binding.method.to_string(),
        declared_by: Some(binding.declared_by.clone()),
        file: binding.file.clone(),
        line: None,
        symbol: binding.symbol.clone(),
    }
}

pub(super) fn relative(repo: &Utf8Path, file: &Utf8Path) -> Utf8PathBuf {
    file.strip_prefix(repo)
        .map_or_else(|_| file.to_path_buf(), Utf8Path::to_path_buf)
}
