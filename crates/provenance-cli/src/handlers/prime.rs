use crate::output::{self, ReportFormat};
use provenance_macros::rule;
use provenance_porcelain::guidance;

/// Introduces Provenance with the shared domain guidance.
#[rule("rule_porcelain_cli_prime_teaches_domain")]
pub(super) fn handle(format: ReportFormat) -> anyhow::Result<()> {
    let guidance = guidance::guidance();
    match format {
        ReportFormat::Markdown => println!("{}", guidance.guidance),
        ReportFormat::Json => output::print_json(&guidance)?,
    }
    Ok(())
}
