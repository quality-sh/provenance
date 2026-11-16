use crate::output;
use provenance_cli::cli_options::flag_name;
use provenance_cli::exposed;
use provenance_macros::rule;
use provenance_porcelain::action::Action;
use provenance_porcelain::guidance::{self, FIRST_STEPS};
use provenance_porcelain::registry::{json_format, Command, PrimeFormat, JSON, SCOPE};

/// The text before the first commands.
const BEFORE_COMMANDS: &str = "\n\nFor a focused start, run:\n";

/// The first commands that the CLI suggests after the shared guidance: the CLI form of
/// each first step, with its arguments, the scope, and the JSON format as flags. The CLI
/// shows search and get in each build, so it gives each first step.
fn first_steps() -> String {
    let mut text = String::from(BEFORE_COMMANDS);
    for step in FIRST_STEPS {
        text.push_str("\n- `provenance ");
        text.push_str(&exposed::form(&step.command.declaration()));
        let arguments = step
            .arguments
            .iter()
            .map(|argument| (flag_name(&argument.field), argument.value()))
            .chain([
                (flag_name(&SCOPE), SCOPE.placeholder()),
                (flag_name(&json_format(None)), JSON.into()),
            ]);
        for (flag, value) in arguments {
            text.push_str(" --");
            text.push_str(&flag);
            text.push(' ');
            text.push_str(&value);
        }
        text.push('`');
    }
    text
}

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
    guidance.guidance.push_str(&first_steps());
    guidance.guidance.extend(record_actions());
    match format {
        PrimeFormat::Markdown => println!("{}", guidance.guidance),
        PrimeFormat::Json => output::print_json(&guidance)?,
    }
    Ok(())
}
