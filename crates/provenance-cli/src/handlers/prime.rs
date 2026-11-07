use crate::output;
use provenance_macros::rule;
use provenance_porcelain::guidance;
use provenance_porcelain::registry::PrimeFormat;

/// The first commands that the CLI suggests after the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, run:\n\n\
- `provenance search --text <text> --scope <scope> --format json`\n\
- `provenance <id> get --view children --depth <N> --scope <scope> --format json`";

/// Introduces Provenance with the shared domain guidance and the CLI first steps.
#[rule("rule_porcelain_cli_prime_teaches_domain")]
pub(super) fn handle(format: PrimeFormat) -> anyhow::Result<()> {
    let mut guidance = guidance::guidance();
    guidance.guidance.push_str(FIRST_STEPS);
    match format {
        PrimeFormat::Markdown => println!("{}", guidance.guidance),
        PrimeFormat::Json => output::print_json(&guidance)?,
    }
    Ok(())
}
