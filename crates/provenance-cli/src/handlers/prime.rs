use crate::output;
use provenance_macros::rule;
use provenance_porcelain::guidance;
use provenance_porcelain::registry::PrimeFormat;

/// Introduces Provenance with the shared domain guidance.
#[rule("rule_porcelain_cli_prime_teaches_domain")]
pub(super) fn handle(format: PrimeFormat) -> anyhow::Result<()> {
    let guidance = guidance::guidance();
    match format {
        PrimeFormat::Markdown => println!("{}", guidance.guidance),
        PrimeFormat::Json => output::print_json(&guidance)?,
    }
    Ok(())
}
