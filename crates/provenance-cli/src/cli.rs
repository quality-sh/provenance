pub mod workspace;

use clap::Parser;

// The root parser. Every command under it comes from the registry. A doc comment here
// would become the about line of `provenance --help`.
#[derive(Parser)]
#[command(
    name = "provenance",
    version,
    subcommand_required = true,
    arg_required_else_help = true,
    after_help = concat!(
        "Dynamic commands:\n",
        "  provenance api [PATH]\n",
        "  provenance api --filter <text>\n",
        "  provenance <id> get --view children --depth <N>\n",
        "  provenance <id> trace\n\n",
        "Discussion actions: provenance discussions, or provenance <record-id> ",
        "discussions|discuss, or provenance <discussion-id> discussion|reply."
    )
)]
pub struct Cli {
    /// Drop the advisory notes commands print alongside their output, such as
    /// the warning that this repository has no shaping skills installed.
    #[arg(long, global = true)]
    pub quiet: bool,
}

/// Which triage state a `dogfood triage list` view shows.
#[cfg(feature = "dogfood")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriageFilter {
    Unhandled,
    Handled,
    All,
}

#[cfg(feature = "dogfood")]
impl TriageFilter {
    /// The filter that one `--status` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        match value {
            "unhandled" => Some(Self::Unhandled),
            "handled" => Some(Self::Handled),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

#[cfg(feature = "dogfood")]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DogfoodCategory {
    Friction,
    Confusion,
    Missing,
    Bug,
    Idea,
}

#[cfg(feature = "dogfood")]
impl DogfoodCategory {
    /// The category that one `--category` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
    }
}

#[cfg(feature = "dogfood")]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DogfoodSeverity {
    Blocked,
    Workaround,
    Annoyance,
}

#[cfg(feature = "dogfood")]
impl DogfoodSeverity {
    /// The severity that one `--severity` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
    }
}
