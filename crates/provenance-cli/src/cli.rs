pub mod dictionary;
pub mod graph;
pub mod ideation;
pub mod report;
pub mod workspace;

pub use ideation::{IdeationArtifactKind, SchemaCommand};

use clap::Parser;

/// How the caller runs provenance, so init prints commands that the caller can run.
#[derive(Clone, Copy, Debug, Default)]
pub enum InvocationChannel {
    #[default]
    Native,
    Typescript,
}

impl InvocationChannel {
    /// The channel that one `--invocation-channel` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "typescript" => Some(Self::Typescript),
            _ => None,
        }
    }
}

/// The package manager that runs the TypeScript package.
#[derive(Clone, Copy, Debug)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Deno,
    Nub,
}

impl PackageManager {
    /// The package manager that one `--package-manager` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        Some(match value {
            "npm" => Self::Npm,
            "pnpm" => Self::Pnpm,
            "yarn" => Self::Yarn,
            "bun" => Self::Bun,
            "deno" => Self::Deno,
            "nub" => Self::Nub,
            _ => return None,
        })
    }
}

/// The root parser. Every command under it comes from the registry.
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
        "  provenance <id> get --view children --depth <N>\n\n",
        "Discussion actions: provenance discussions [<discussion-id> get], or provenance ",
        "<record-id> discussions|discuss, or provenance <discussion-id> reply."
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
