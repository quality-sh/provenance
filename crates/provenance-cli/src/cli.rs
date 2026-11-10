use clap::Parser;

/// The build identity: the release number, then the commit for a build from a
/// git checkout, for example `0.4.0 (6347b4b0)`.
pub const BUILD: &str = env!("PROVENANCE_BUILD");

// The root parser. Every command under it comes from the registry. A doc comment here
// would become the about line of `provenance --help`.
#[derive(Parser)]
#[command(
    name = "provenance",
    version = BUILD,
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
