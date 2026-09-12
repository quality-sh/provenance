//! The reviewed diagnostic-code catalog.
//!
//! Codes are stable identifiers. Every prescribed next action is fixed prose
//! in this catalog; the renderer never generates advice from graph text. A
//! code added here is reviewed text, so the report stays trusted.

/// Stable diagnostic codes for report findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// An active Rule has no current verification binding.
    ActiveRuleMissingVerification,
    /// A deprecated or archived Rule still has a current binding.
    InactiveRuleCurrentBinding,
    /// A verification site is gone while other evidence may remain.
    VerificationSiteRemoved,
    /// A verification site moved to a new location.
    VerificationSiteMoved,
    /// A Requirement statement changed, so evidence may need review.
    RequirementStatementChanged,
}

impl DiagnosticCode {
    /// Parse one catalog code. Unknown codes never reach the report.
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "active_rule_missing_verification" => Some(Self::ActiveRuleMissingVerification),
            "inactive_rule_current_binding" => Some(Self::InactiveRuleCurrentBinding),
            "verification_site_removed" => Some(Self::VerificationSiteRemoved),
            "verification_site_moved" => Some(Self::VerificationSiteMoved),
            "requirement_statement_changed" => Some(Self::RequirementStatementChanged),
            _ => None,
        }
    }

    /// The stable code string. Round-trips with [`Self::parse`].
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveRuleMissingVerification => "active_rule_missing_verification",
            Self::InactiveRuleCurrentBinding => "inactive_rule_current_binding",
            Self::VerificationSiteRemoved => "verification_site_removed",
            Self::VerificationSiteMoved => "verification_site_moved",
            Self::RequirementStatementChanged => "requirement_statement_changed",
        }
    }

    /// The stable finding headline, readable in one line.
    pub const fn headline(self) -> &'static str {
        match self {
            Self::ActiveRuleMissingVerification => "no current verification binding found",
            Self::InactiveRuleCurrentBinding => "a retired Rule still has current bindings",
            Self::VerificationSiteRemoved => "one verification site is gone",
            Self::VerificationSiteMoved => "verification evidence moved",
            Self::RequirementStatementChanged => "the intent changed; evidence needs review",
        }
    }

    /// The prescribed next action. Reviewed catalog text, never graph text.
    pub const fn next_action(self) -> &'static str {
        match self {
            Self::ActiveRuleMissingVerification => {
                "Add evidence for the full-scan absence behavior, or inspect an \
                 intended qualified or wrapped marker if evidence already exists. \
                 Rerun the full supported scan. Do not add an empty assertion or \
                 change the lifecycle to silence this report."
            }
            Self::InactiveRuleCurrentBinding => {
                "Confirm whether the lifecycle change was intended. If it was, \
                 remove or rebind the current sites as the approved replacement \
                 requires. If it was not, correct the lifecycle edit. Do not \
                 invent a replacement obligation."
            }
            Self::VerificationSiteRemoved => {
                "Restore the removed check, or identify replacement evidence that \
                 checks the same behavior. Review the assertions; a replacement \
                 marker alone does not establish the same evidence."
            }
            Self::VerificationSiteMoved => {
                "Inspect the moved assertions and the actual run result when it is \
                 available. A relocation alone does not create an absence finding. \
                 If several sites match, list the candidates instead of choosing \
                 one."
            }
            Self::RequirementStatementChanged => {
                "Resolve the statement mismatch and inspect the boundary assertions \
                 before accepting the change. A cleared review record or a pass of \
                 unchanged assertions does not establish support for the new \
                 statement."
            }
        }
    }
}
