use crate::cli;
use crate::output::{self, JsonFormat};
use crate::registry_cli::BoundContext;
use camino::Utf8PathBuf;
use provenance_cli::servers::{DocsServeInput, McpInput, ReviewInput, Server};
use provenance_porcelain::export::Document;
use provenance_porcelain::init::InitReport;
use provenance_porcelain::registry::{
    self, CargoInitInput, DictionaryImportInput, DictionaryStatusInput, ExportInput, GraphCommand,
    ImportInput, InitInput, MaterializeInput, MergeJsonlInput, ReferenceInput, ReferenceIssueInput,
    ReportBuildInput, ReportRenderInput, RepositoryCommand, SchemaShowInput, SkillsInstallInput,
    SkillsListInput, SkillsShowInput, SwarmBacktraceLandInput, ValidateInput, WorkspaceCommand,
};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::{ArtifactPort, RepositoryPort, SetupPort};
use provenance_transport::repository::RepoContext;

mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod mcp;
mod merge_jsonl;
mod prime;

#[cfg(feature = "dogfood")]
#[allow(clippy::redundant_pub_crate)]
pub(super) use dogfood::{handle as dogfood_command, DogfoodInput};

/// Runs the check command with the flags that its declaration names.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn check_command(
    input: provenance_porcelain::check::CheckInput,
    context: crate::registry_cli::BoundContext,
) -> anyhow::Result<()> {
    provenance_cli::porcelain::dispatch_check(&context.repo, context.json, input).await
}

/// Prints the shared domain guidance.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn prime_command(input: &registry::PrimeInput) -> anyhow::Result<()> {
    prime::handle(input.format)
}

/// The typed input of one repository command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum RepositoryInput {
    Init(InitInput),
    CargoInit(CargoInitInput),
    Materialize(MaterializeInput, BoundContext),
    Export(ExportInput, BoundContext),
    Import(ImportInput, BoundContext),
    MergeJsonl(MergeJsonlInput),
    Validate(ValidateInput),
    SchemaShow(SchemaShowInput),
    DictionaryImport(DictionaryImportInput, BoundContext),
    DictionaryStatus(DictionaryStatusInput, BoundContext),
}

impl RepositoryInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: RepositoryCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = registry::Command::Repository(command).declaration();
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
            ($variant:ident, bound) => {
                Self::$variant(
                    crate::registry_cli::input(&declaration, matches)?,
                    crate::registry_cli::bound_context(matches),
                )
            };
        }
        Ok(match command {
            RepositoryCommand::Init => read!(Init),
            RepositoryCommand::CargoInit => read!(CargoInit),
            RepositoryCommand::Materialize => read!(Materialize, bound),
            RepositoryCommand::Export => read!(Export, bound),
            RepositoryCommand::Import => read!(Import, bound),
            RepositoryCommand::MergeJsonl => read!(MergeJsonl),
            RepositoryCommand::Validate => read!(Validate),
            RepositoryCommand::SchemaShow => read!(SchemaShow),
            RepositoryCommand::DictionaryImport => read!(DictionaryImport, bound),
            RepositoryCommand::DictionaryStatus => read!(DictionaryStatus, bound),
        })
    }
}

/// Runs one repository command.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn repository_command(input: RepositoryInput, quiet: bool) -> anyhow::Result<()> {
    match input {
        RepositoryInput::Init(input) => print_init(&setup().init(input).await?, quiet),
        RepositoryInput::CargoInit(input) => print_init(&setup().cargo_init(input).await?, quiet),
        RepositoryInput::Materialize(input, context) => {
            let json = context.json;
            let result = work(context).materialize(input).await?;
            print(
                json,
                &result,
                provenance_porcelain::materialize::render_readable,
            )
        }
        RepositoryInput::Export(input, context) => {
            let to_file = input.output.is_some();
            print_document(to_file, &work(context).export(input).await?);
            Ok(())
        }
        RepositoryInput::Import(input, context) => {
            let json = context.json;
            let result = work(context).import(input).await?;
            print(json, &result, provenance_porcelain::import::render_readable)
        }
        RepositoryInput::MergeJsonl(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            let path = input.path.map(Utf8PathBuf::from);
            merge_jsonl::handle(
                &input.base.into(),
                &input.ours.into(),
                &input.theirs.into(),
                input.output.map(Utf8PathBuf::from),
                path.as_deref(),
            )
        }
        RepositoryInput::Validate(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            output::print_json(&Porcelain::new(ArtifactPort).validate(input).await?)
        }
        RepositoryInput::SchemaShow(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            output::print_json(&Porcelain::new(ArtifactPort).schema_show(input).await?)
        }
        RepositoryInput::DictionaryImport(input, context) => {
            let json = context.json;
            let result = work(context).dictionary_import(input).await?;
            print(
                json,
                &result,
                provenance_porcelain::dictionary::render_import_readable,
            )
        }
        RepositoryInput::DictionaryStatus(input, context) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            output::print_json(&work(context).dictionary_status(input).await?)
        }
    }
}

/// Prints `result` as readable text, or as JSON when the caller asked for it.
fn print<T: serde::Serialize>(
    json: bool,
    result: &T,
    readable: fn(&T) -> String,
) -> anyhow::Result<()> {
    if json {
        output::print_json(result)
    } else {
        println!("{}", readable(result));
        Ok(())
    }
}

/// The adapter of init, cargo init, and the skills commands, with the bundled skills.
const fn setup() -> Porcelain<SetupPort> {
    Porcelain::new(SetupPort::new(crate::skills::BUNDLED))
}

/// Prints the notices of an init on standard error, then its summary unless the caller
/// asked for silence.
fn print_init(report: &InitReport, quiet: bool) -> anyhow::Result<()> {
    use std::io::Write;
    for notice in &report.notices {
        eprintln!("{notice}");
    }
    if !quiet {
        std::io::stdout()
            .lock()
            .write_all(report.summary.as_bytes())?;
    }
    Ok(())
}

/// Prints a document read, unless the read wrote it to the output file of its input.
fn print_document(to_file: bool, document: &Document) {
    if !to_file {
        print!("{}", document.text);
    }
}

/// The repository commands on the repository and scope of the CLI options.
fn work(context: BoundContext) -> Porcelain<RepositoryPort> {
    Porcelain::new(RepositoryPort::new(RepoContext::new(
        context.repo,
        context.scope,
    )))
}

/// The typed input of one graph command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum GraphInput {
    ReferenceIssue(ReferenceIssueInput, BoundContext),
    ReferenceShow(ReferenceInput, BoundContext),
    ReferenceVerify(ReferenceInput, BoundContext),
    ReferenceExactExport(ReferenceInput, BoundContext),
    SwarmBacktraceLand(SwarmBacktraceLandInput, BoundContext),
}

impl GraphInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: GraphCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = registry::Command::Graph(command).declaration();
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(
                    crate::registry_cli::input(&declaration, matches)?,
                    crate::registry_cli::bound_context(matches),
                )
            };
        }
        Ok(match command {
            GraphCommand::ReferenceIssue => read!(ReferenceIssue),
            GraphCommand::ReferenceShow => read!(ReferenceShow),
            GraphCommand::ReferenceVerify => read!(ReferenceVerify),
            GraphCommand::ReferenceExactExport => read!(ReferenceExactExport),
            GraphCommand::SwarmBacktraceLand => read!(SwarmBacktraceLand),
        })
    }
}

/// Runs one graph command.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn graph_command(input: GraphInput) -> anyhow::Result<()> {
    match input {
        GraphInput::ReferenceIssue(input, context) => {
            output::print_json(&work(context).reference_issue(input).await?)
        }
        GraphInput::ReferenceShow(input, context) => {
            output::print_json(&work(context).reference_show(input).await?)
        }
        GraphInput::ReferenceVerify(input, context) => {
            output::print_json(&work(context).reference_verify(input).await?)
        }
        GraphInput::ReferenceExactExport(input, context) => {
            output::print_json(&work(context).reference_exact_export(input).await?)
        }
        GraphInput::SwarmBacktraceLand(input, context) => {
            let json = context.json;
            let result = work(context).swarm_backtrace_land(input).await?;
            print(
                json,
                &result,
                provenance_porcelain::swarm_backtrace::render_readable,
            )
        }
    }
}

/// The value that one declared flag names. The CLI parser accepts only declared values.
fn flag_value<T>(value: &str, parse: impl FnOnce(&str) -> Option<T>) -> anyhow::Result<T> {
    parse(value).ok_or_else(|| anyhow::anyhow!("unsupported value {value}"))
}

/// The typed input of one workspace command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum WorkspaceInput {
    SkillsList(SkillsListInput),
    SkillsShow(SkillsShowInput),
    SkillsInstall(SkillsInstallInput),
    ReportRender(ReportRenderInput),
    ReportBuild(ReportBuildInput, BoundContext),
}

impl WorkspaceInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: WorkspaceCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = registry::Command::Workspace(command).declaration();
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
            ($variant:ident, bound) => {
                Self::$variant(
                    crate::registry_cli::input(&declaration, matches)?,
                    crate::registry_cli::bound_context(matches),
                )
            };
        }
        Ok(match command {
            WorkspaceCommand::SkillsList => read!(SkillsList),
            WorkspaceCommand::SkillsShow => read!(SkillsShow),
            WorkspaceCommand::SkillsInstall => read!(SkillsInstall),
            WorkspaceCommand::ReportRender => read!(ReportRender),
            WorkspaceCommand::ReportBuild => read!(ReportBuild, bound),
        })
    }
}

/// Runs one workspace command.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn workspace_command(input: WorkspaceInput) -> anyhow::Result<()> {
    match input {
        WorkspaceInput::SkillsList(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            output::print_json(&setup().skills_list(input).await?)
        }
        WorkspaceInput::SkillsShow(input) => {
            print!("{}", setup().skills_show(input).await?.text);
            Ok(())
        }
        WorkspaceInput::SkillsInstall(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            output::print_json(&setup().skills_install(input).await?)
        }
        WorkspaceInput::ReportRender(input) => {
            let to_file = input.output.is_some();
            print_document(
                to_file,
                &Porcelain::new(ArtifactPort).report_render(input).await?,
            );
            Ok(())
        }
        WorkspaceInput::ReportBuild(input, context) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            let to_file = input.output.is_some();
            print_document(to_file, &work(context).report_build(input).await?);
            Ok(())
        }
    }
}

/// The typed input of one server command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum ServerInput {
    DocsServe(DocsServeInput),
    Review(ReviewInput),
    Mcp(McpInput),
}

impl ServerInput {
    /// Reads the input of `server` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(server: Server, matches: &clap::ArgMatches) -> anyhow::Result<Self> {
        let declaration = server.declaration();
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
        }
        Ok(match server {
            Server::DocsServe => read!(DocsServe),
            Server::Review => read!(Review),
            Server::Mcp => read!(Mcp),
        })
    }
}

/// Runs one server command until it stops.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn server_command(input: ServerInput) -> anyhow::Result<()> {
    match input {
        ServerInput::DocsServe(input) => {
            docs::handle(cli::workspace::DocsCommand::Serve {
                repo: input.repo.into(),
                host: input.host,
                port: input.port,
            })
            .await
        }
        ServerInput::Review(input) => {
            crate::review::run(crate::review::Options {
                repo: input.repo.into(),
                repository_id: input.repository_id,
                scope: input.scope,
                port: input.port,
                no_open: input.no_open,
                detach: input.detach,
            })
            .await
        }
        ServerInput::Mcp(input) => mcp::handle(input).await,
    }
}

/// Runs a synchronous handler on a blocking thread and waits for it.
async fn run_blocking(
    handler: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(handler).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value names one variant of the input type of its field.
    fn names<T: serde::de::DeserializeOwned>(value: &str) -> bool {
        serde_json::from_value::<T>(serde_json::Value::from(value)).is_ok()
    }

    /// Implementation aid: each value that a CLI-only registry flag declares converts to
    /// the CLI value that its handler takes.
    #[test]
    fn every_declared_repository_value_converts() {
        for command in registry::Command::ALL {
            let cli_only = matches!(
                command,
                registry::Command::Repository(_)
                    | registry::Command::Graph(_)
                    | registry::Command::Workspace(_)
                    | registry::Command::Dogfood(_)
            );
            if !cli_only || !provenance_cli::exposed::shows(*command) {
                continue;
            }
            for flag in command.declaration().input.fields() {
                for value in flag.values {
                    let converts = match flag.name {
                        "format"
                            if *command
                                == registry::Command::Repository(RepositoryCommand::Export) =>
                        {
                            names::<provenance_porcelain::export::ExportFormat>(value)
                        }
                        "format"
                            if *command
                                == registry::Command::Workspace(WorkspaceCommand::ReportRender) =>
                        {
                            names::<provenance_porcelain::report::ReportFormat>(value)
                        }
                        "format" => JsonFormat::from_flag(value).is_some(),
                        "artifact" => names::<provenance_porcelain::schema::Artifact>(value),
                        "invocation_channel" => {
                            names::<provenance_porcelain::init::InvocationChannel>(value)
                        }
                        "package_manager" => {
                            names::<provenance_porcelain::init::PackageManager>(value)
                        }
                        #[cfg(feature = "dogfood")]
                        "category" => cli::DogfoodCategory::from_flag(value).is_some(),
                        #[cfg(feature = "dogfood")]
                        "severity" => cli::DogfoodSeverity::from_flag(value).is_some(),
                        #[cfg(feature = "dogfood")]
                        "status" => cli::TriageFilter::from_flag(value).is_some(),
                        other => panic!("{command:?} field {other} has no conversion"),
                    };
                    assert!(converts, "{command:?} field {} {value}", flag.name);
                }
            }
        }
    }
}
