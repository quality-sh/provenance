use crate::output;
use provenance_cli::cli_options::flag_name;
use provenance_cli::exposed;
use provenance_macros::rule;
use provenance_porcelain::action::Action;
use provenance_porcelain::guidance;
use provenance_porcelain::registry::{json_format, Command, FieldValue, PrimeFormat, SCOPE};

/// The text before the first commands, and the text before and after each command.
const FIRST_STEPS: (&str, &str, &str) =
    ("\n\nFor a focused start, run:\n", "\n- `provenance ", "`");

/// The first commands that the CLI suggests after the shared guidance, when the CLI shows
/// their commands: the CLI form of each first step, its fields as flags, and the scope and
/// the JSON format.
fn first_steps() -> String {
    let format = json_format(None);
    let options = [(SCOPE, None), (format, format.values.first().copied())];
    let lines: Vec<String> = guidance::first_steps(exposed::shows)
        .map(|(command, fields)| {
            let flags = fields.iter().chain(&options).map(|(field, value)| {
                let value = match (value, field.value) {
                    (Some(value), _) => (*value).to_owned(),
                    (None, FieldValue::Count) => "<N>".to_owned(),
                    (None, _) => format!("<{}>", field.name),
                };
                format!(" --{} {value}", flag_name(field))
            });
            let (_, start, end) = FIRST_STEPS;
            let form = exposed::form(&command.declaration());
            format!("{start}{form}{}{end}", flags.collect::<String>())
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let (before, _, _) = FIRST_STEPS;
    format!("{before}{}", lines.concat())
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
