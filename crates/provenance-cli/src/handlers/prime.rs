use crate::output;
use provenance_cli::exposed;
use provenance_macros::rule;
use provenance_porcelain::action::Action;
use provenance_porcelain::guidance;
use provenance_porcelain::registry::{Command, PrimeFormat};

/// The first commands that the CLI suggests after the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, run:\n\n\
- `provenance search --text <text> --scope <scope> --format json`\n\
- `provenance <id> get --view children --depth <N> --scope <scope> --format json`";

/// The CLI form of an action on a record, with the update and create actions, when the
/// CLI shows them.
fn record_actions() -> Option<String> {
    let shown = |action| {
        let command = Command::Action(action);
        exposed::shows(command).then(|| exposed::form(&command.declaration()))
    };
    let update = shown(Action::Update)?;
    let create = shown(Action::Create)?;
    Some(format!(
        "\n\nTo do an action on a record, put the action after the record ID, as in \
         `provenance {update}`. An action that makes a record also needs its type, as in \
         `provenance {create} --type requirement`. Add `--help` to see the fields of an \
         action."
    ))
}

/// Introduces Provenance with the guidance of the commands that the CLI shows, the CLI
/// first steps, and the CLI form of an action on a record.
#[rule("rule_porcelain_cli_prime_teaches_domain")]
pub(super) fn handle(format: PrimeFormat) -> anyhow::Result<()> {
    let mut guidance = guidance::guidance_for(&exposed::shown(), &[], None);
    guidance.guidance.push_str(FIRST_STEPS);
    guidance.guidance.extend(record_actions());
    match format {
        PrimeFormat::Markdown => println!("{}", guidance.guidance),
        PrimeFormat::Json => output::print_json(&guidance)?,
    }
    Ok(())
}
