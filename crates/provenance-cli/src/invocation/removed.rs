//! CLI commands that were removed, and the command that replaces each.

use provenance_macros::rule;

/// The words of each removed command and the command that replaces it.
const REMOVED: &[(&[&str], &str)] = &[
    (&["graph"], "provenance <id> get --view children"),
    (&["traceability"], "provenance <id> trace"),
    (&["gaps"], "provenance check --gaps"),
    (&["health"], "provenance check --gaps"),
    (&["orphans"], "provenance check --gaps"),
    (&["coverage", "scan"], "provenance check --bindings"),
    (&["docs", "check"], "provenance check --docs"),
];

/// Refuses a removed command and names the command that replaces it.
#[rule("rule_cli_removed_command_names_replacement")]
pub(super) fn refuse(arguments: &[String]) -> anyhow::Result<()> {
    let common = crate::registry_cli::with_global_flags(clap::Command::new("provenance"));
    for (words, replacement) in REMOVED {
        let used = words.iter().enumerate().all(|(index, word)| {
            super::grammar::positional_word(arguments, &common, index + 1) == Some(*word)
        });
        if used {
            anyhow::bail!(
                "`provenance {}` was removed. Use `{replacement}`",
                words.join(" ")
            );
        }
    }
    Ok(())
}
