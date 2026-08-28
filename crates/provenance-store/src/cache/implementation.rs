//! Whether the repository shows code behind a Rule.
//!
//! Unimplemented is absence. It is derived on every read from the same two
//! directions the coverage scan reads — scanner sites and canonical
//! implementation bindings — and it is never written into the record. A Rule
//! that no code implements is still a Rule. The state says where the work
//! stands, not whether the obligation is real.

use std::collections::BTreeSet;

use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::{Rule, RuleStatus, ScopeId};
use provenance_scanner::{source_sites, SourceSiteRole};

/// Where a Rule stands against the code, derived and never stored.
///
/// `NotExpected` is not a weaker `Unimplemented`. A draft Rule is not accepted
/// yet, so the repository owes it no code, and calling that absence
/// `Unimplemented` would report work that nobody agreed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationState {
    /// A scanner site or a canonical binding names the code that realizes it.
    Implemented,
    /// An active Rule that no code implements yet. This is an ordinary state
    /// of a planned obligation. It is not a defect, and it is not a reason to
    /// doubt the Rule.
    Unimplemented,
    /// The Rule is not active, so no implementation is expected of it yet.
    NotExpected,
}

impl ImplementationState {
    /// The words an agent-facing report prints for this state.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::Unimplemented => "unimplemented",
            Self::NotExpected => "implementation not expected yet",
        }
    }
}

/// The rule ids that the repository has an implementation for, read once.
///
/// Both surfaces that report this state ask about many Rules against one
/// repository. The scan and the binding read happen once here, and not once
/// for each Rule.
pub struct ImplementationIndex {
    implemented: BTreeSet<String>,
}

impl ImplementationIndex {
    pub fn build(
        layout: &ProvenanceLayout,
        store: &StateStore,
        scope: &ScopeId,
    ) -> anyhow::Result<Self> {
        let scans = provenance_scanner::scan_path(layout.root())?;
        let mut implemented = source_sites(&scans)
            .filter(|site| site.role() == SourceSiteRole::Implementation)
            .map(|site| site.rule_id().to_string())
            .collect::<BTreeSet<_>>();
        implemented.extend(
            store
                .active_implementation_bindings(scope)?
                .iter()
                .map(|binding| binding.rule_id.as_str().to_string()),
        );
        Ok(Self { implemented })
    }

    pub fn state(&self, rule: &Rule) -> ImplementationState {
        if self.implemented.contains(rule.id.as_str()) {
            ImplementationState::Implemented
        } else if rule.status == RuleStatus::Active && !rule.retired {
            ImplementationState::Unimplemented
        } else {
            ImplementationState::NotExpected
        }
    }
}

/// The paragraph that every agent-facing report prints beside these states.
///
/// A reader who meets `unimplemented` without it sees an absence and no way to
/// read it. That is how a planning-first graph gets called invented.
pub const ABSENCE_NOTE: &str = "Unimplemented and Unverified are absence. \
Provenance derives them on each read and stores neither. A Rule can be active \
before code implements it. Audit a Rule against its sources, its Requirements, \
and the decisions that produced it. The absence of code is not evidence that a \
Rule is invalid.";
