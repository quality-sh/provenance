use crate::cli;
use crate::output::{self, JsonFormat, OutputFormat};
use camino::Utf8PathBuf;
use provenance_cli::cli_form::{self, CheckCliInput};
use provenance_cli::servers::{DocsServeInput, McpInput, ReviewInput, Server};
use provenance_porcelain::registry::{
    self, CargoInitInput, DictionaryImportInput, DictionaryStatusInput, ExportInput, GraphCommand,
    ImportInput, InitInput, MaterializeInput, MergeJsonlInput, ReferenceInput, ReferenceIssueInput,
    ReportBuildInput, ReportRenderInput, RepositoryCommand, SchemaShowInput, SkillsInstallInput,
    SkillsListInput, SkillsShowInput, SwarmBacktraceLandInput, ValidateInput, WorkspaceCommand,
};
use provenance_store::cache;
use provenance_transport::repository::RepoContext;

mod cargo_init;
mod dictionary;
mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod export;
mod graph_reference;
mod import;
mod mcp;
mod merge_jsonl;
mod prime;
mod repo;
mod report;
mod schema;
mod skills;
mod swarm_backtrace;
mod validate;

#[cfg(feature = "dogfood")]
#[allow(clippy::redundant_pub_crate)]
pub(super) use dogfood::{handle as dogfood_command, DogfoodInput};

/// Runs the check command with the flags that its declaration names.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn check_command(input: CheckCliInput) -> anyhow::Result<()> {
    provenance_cli::porcelain::dispatch_check(input).await
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
    Materialize(MaterializeInput),
    Export(ExportInput),
    Import(ImportInput),
    MergeJsonl(MergeJsonlInput),
    Validate(ValidateInput),
    SchemaShow(SchemaShowInput),
    DictionaryImport(DictionaryImportInput),
    DictionaryStatus(DictionaryStatusInput),
}

impl RepositoryInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: RepositoryCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = cli_form::declaration(registry::Command::Repository(command));
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
        }
        Ok(match command {
            RepositoryCommand::Init => read!(Init),
            RepositoryCommand::CargoInit => read!(CargoInit),
            RepositoryCommand::Materialize => read!(Materialize),
            RepositoryCommand::Export => read!(Export),
            RepositoryCommand::Import => read!(Import),
            RepositoryCommand::MergeJsonl => read!(MergeJsonl),
            RepositoryCommand::Validate => read!(Validate),
            RepositoryCommand::SchemaShow => read!(SchemaShow),
            RepositoryCommand::DictionaryImport => read!(DictionaryImport),
            RepositoryCommand::DictionaryStatus => read!(DictionaryStatus),
        })
    }
}

/// Runs one repository command.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn repository_command(input: RepositoryInput, quiet: bool) -> anyhow::Result<()> {
    match input {
        RepositoryInput::Init(input) => {
            let options = repo::InitOptions {
                scope: input.scope,
                path_prefix: input.path_prefix.map(Utf8PathBuf::from),
                disposition_actor_ids: input.disposition_actor_id,
                clear_disposition_actors: input.clear_disposition_actors,
                ste_pdf: input.ste_pdf.map(Utf8PathBuf::from),
                invocation_channel: flag_value(
                    &input.invocation_channel,
                    cli::InvocationChannel::from_flag,
                )?,
                package_manager: input
                    .package_manager
                    .as_deref()
                    .map(|manager| flag_value(manager, cli::PackageManager::from_flag))
                    .transpose()?,
                quiet,
            };
            let path = Utf8PathBuf::from(input.path);
            run_blocking(move || repo::init(&path, options)).await
        }
        RepositoryInput::CargoInit(input) => {
            let ste_pdf = input.ste_pdf.map(Utf8PathBuf::from);
            run_blocking(move || cargo_init::handle(input.package.as_deref(), ste_pdf, quiet)).await
        }
        RepositoryInput::Materialize(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            let repository = RepoContext::new(input.repo, "default").open()?;
            output::print_json(&cache::materialize_state(&repository).await?)
        }
        RepositoryInput::Export(input) => export::handle(
            &RepoContext::new(input.repo, input.scope),
            flag_value(&input.format, OutputFormat::from_flag)?,
            input.output.map(Utf8PathBuf::from),
        ),
        RepositoryInput::Import(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            import::handle(
                &RepoContext::new(input.repo, input.scope),
                Utf8PathBuf::from(input.input),
                input.dry_run,
            )
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
            validate::handle(
                artifact(&input.artifact)?,
                camino::Utf8Path::new(&input.input),
            )
        }
        RepositoryInput::SchemaShow(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            schema::handle(cli::SchemaCommand::Show {
                artifact: artifact(&input.artifact)?,
            })
        }
        RepositoryInput::DictionaryImport(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            dictionary::handle(cli::dictionary::DictionaryCommand::Import {
                pdf: input.pdf.into(),
                repo: input.repo.into(),
            })
        }
        RepositoryInput::DictionaryStatus(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            dictionary::handle(cli::dictionary::DictionaryCommand::Status {
                repo: input.repo.into(),
            })
        }
    }
}

/// The typed input of one graph command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum GraphInput {
    ReferenceIssue(ReferenceIssueInput),
    ReferenceShow(ReferenceInput),
    ReferenceVerify(ReferenceInput),
    ReferenceExactExport(ReferenceInput),
    SwarmBacktraceLand(SwarmBacktraceLandInput),
}

impl GraphInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: GraphCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = cli_form::declaration(registry::Command::Graph(command));
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
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

/// Runs one graph command on the calling thread.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn graph_command(input: GraphInput) -> anyhow::Result<()> {
    use cli::graph::GraphReferenceCommand;
    let reference = |input: ReferenceInput| (input.repo.into(), input.reference.into());
    match input {
        GraphInput::ReferenceIssue(input) => {
            graph_reference::handle(GraphReferenceCommand::Issue {
                context: RepoContext::new(input.repo, input.scope),
                commit: input.commit,
                correlation_system: input.correlation_system,
                correlation_key: input.correlation_key,
            })
        }
        GraphInput::ReferenceShow(input) => {
            let (repo, reference) = reference(input);
            graph_reference::handle(GraphReferenceCommand::Show { repo, reference })
        }
        GraphInput::ReferenceVerify(input) => {
            let (repo, reference) = reference(input);
            graph_reference::handle(GraphReferenceCommand::Verify { repo, reference })
        }
        GraphInput::ReferenceExactExport(input) => {
            let (repo, reference) = reference(input);
            graph_reference::handle(GraphReferenceCommand::ExactExport { repo, reference })
        }
        GraphInput::SwarmBacktraceLand(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            swarm_backtrace::handle(cli::ideation::SwarmBacktraceCommand::Land {
                context: RepoContext::new(input.repo, input.scope),
                run_dir: input.run_dir.into(),
                replace: input.replace,
            })
        }
    }
}

/// The value that one declared flag names. The CLI parser accepts only declared values.
fn flag_value<T>(value: &str, parse: impl FnOnce(&str) -> Option<T>) -> anyhow::Result<T> {
    parse(value).ok_or_else(|| anyhow::anyhow!("unsupported value {value}"))
}

fn artifact(value: &str) -> anyhow::Result<cli::IdeationArtifactKind> {
    flag_value(value, cli::IdeationArtifactKind::from_name)
}

/// The typed input of one workspace command, read from its declared flags.
#[allow(clippy::redundant_pub_crate)]
pub(super) enum WorkspaceInput {
    SkillsList(SkillsListInput),
    SkillsShow(SkillsShowInput),
    SkillsInstall(SkillsInstallInput),
    ReportRender(ReportRenderInput),
    ReportBuild(ReportBuildInput),
}

impl WorkspaceInput {
    /// Reads the input of `command` from the matches of its parser.
    #[allow(clippy::redundant_pub_crate)]
    pub(super) fn from_matches(
        command: WorkspaceCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = cli_form::declaration(registry::Command::Workspace(command));
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
        }
        Ok(match command {
            WorkspaceCommand::SkillsList => read!(SkillsList),
            WorkspaceCommand::SkillsShow => read!(SkillsShow),
            WorkspaceCommand::SkillsInstall => read!(SkillsInstall),
            WorkspaceCommand::ReportRender => read!(ReportRender),
            WorkspaceCommand::ReportBuild => read!(ReportBuild),
        })
    }
}

/// Runs one workspace command.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn workspace_command(input: WorkspaceInput) -> anyhow::Result<()> {
    use cli::{report::ReportCommand, workspace::SkillsCommand};
    match input {
        WorkspaceInput::SkillsList(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            skills::handle(SkillsCommand::List)
        }
        WorkspaceInput::SkillsShow(input) => {
            skills::handle(SkillsCommand::Show { name: input.name })
        }
        WorkspaceInput::SkillsInstall(input) => {
            flag_value(&input.format, JsonFormat::from_flag)?;
            skills::handle(SkillsCommand::Install {
                global: input.global,
                copy: input.copy,
                force: input.force,
            })
        }
        WorkspaceInput::ReportRender(input) => report::handle(ReportCommand::Render {
            input: input.input.into(),
            format: flag_value(&input.format, output::ReportFormat::from_flag)?,
            output: input.output.map(Utf8PathBuf::from),
        }),
        WorkspaceInput::ReportBuild(input) => report::handle(ReportCommand::Build {
            repo: input.repo.into(),
            base: input.base,
            head: input.head,
            repository: input.repository,
            scope: input.scope,
            path: input.path.map(Utf8PathBuf::from),
            output: input.output.map(Utf8PathBuf::from),
        }),
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
            for flag in cli_form::form(*command).flags {
                for value in flag.values {
                    let converts = match flag.name {
                        "format"
                            if *command
                                == registry::Command::Repository(RepositoryCommand::Export) =>
                        {
                            OutputFormat::from_flag(value).is_some()
                        }
                        "format"
                            if *command
                                == registry::Command::Workspace(WorkspaceCommand::ReportRender) =>
                        {
                            output::ReportFormat::from_flag(value).is_some()
                        }
                        "format" => JsonFormat::from_flag(value).is_some(),
                        "artifact" => artifact(value).is_ok(),
                        "invocation-channel" => cli::InvocationChannel::from_flag(value).is_some(),
                        "package-manager" => cli::PackageManager::from_flag(value).is_some(),
                        #[cfg(feature = "dogfood")]
                        "category" => cli::DogfoodCategory::from_flag(value).is_some(),
                        #[cfg(feature = "dogfood")]
                        "severity" => cli::DogfoodSeverity::from_flag(value).is_some(),
                        #[cfg(feature = "dogfood")]
                        "status" => cli::TriageFilter::from_flag(value).is_some(),
                        other => panic!("{command:?} --{other} has no conversion"),
                    };
                    assert!(converts, "{command:?} --{} {value}", flag.name);
                }
            }
        }
    }
}
