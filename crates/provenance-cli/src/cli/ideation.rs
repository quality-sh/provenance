use crate::output::JsonFormat;
use camino::Utf8PathBuf;
use clap::{Subcommand, ValueEnum};
use provenance_cli::repo_context::RepoContext;

#[derive(Subcommand)]
pub enum SwarmBacktraceCommand {
    Land {
        #[command(flatten)]
        context: RepoContext,
        #[arg(long)]
        run_dir: Utf8PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum IdeationArtifactKind {
    Contribution,
    SynthesisPacket,
    Proposal,
    Assertion,
    Disposition,
    GraphReference,
    GraphReferenceExport,
}

impl IdeationArtifactKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contribution => "contribution",
            Self::SynthesisPacket => "synthesis-packet",
            Self::Proposal => "proposal",
            Self::Assertion => "assertion",
            Self::Disposition => "disposition",
            Self::GraphReference => "graph-reference",
            Self::GraphReferenceExport => "graph-reference-export",
        }
    }
}

#[derive(Clone, Copy, Subcommand)]
pub enum SchemaCommand {
    Show {
        artifact: IdeationArtifactKind,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}
