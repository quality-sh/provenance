use crate::output::JsonFormat;
use camino::Utf8PathBuf;
use provenance_cli::repo_context::RepoContext;

/// The `provenance swarm-backtrace land` inputs.
pub enum SwarmBacktraceCommand {
    Land {
        context: RepoContext,
        run_dir: Utf8PathBuf,
        replace: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub const ALL: [Self; 7] = [
        Self::Contribution,
        Self::SynthesisPacket,
        Self::Proposal,
        Self::Assertion,
        Self::Disposition,
        Self::GraphReference,
        Self::GraphReferenceExport,
    ];

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

    /// The artifact kind with this name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// The `provenance schema show` input.
#[derive(Clone, Copy)]
pub enum SchemaCommand {
    Show {
        artifact: IdeationArtifactKind,
        format: JsonFormat,
    },
}
