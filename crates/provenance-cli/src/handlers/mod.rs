use crate::cli::{self, Command};
use crate::output::{self, JsonFormat, OutputFormat};
use camino::Utf8PathBuf;
use provenance_cli::repo_context::RepoContext;
use provenance_core::StableId;
use provenance_porcelain::registry::{
    self, CargoInitInput, DictionaryImportInput, DictionaryStatusInput, ExportInput, ImportInput,
    InitInput, MaterializeInput, MergeJsonlInput, RepositoryCommand, SchemaShowInput,
    ValidateInput,
};
use provenance_store::cache;

mod cargo_init;
pub mod check;
mod coverage;
mod dictionary;
mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod export;
mod graph_reference;
mod import;
mod merge_jsonl;
mod prime;
mod repo;
mod report;
mod schema;
mod skills;
mod swarm_backtrace;
mod validate;

/// Runs the check command with the flags that its declaration names.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn check_command(
    input: provenance_porcelain::registry::CheckCliInput,
) -> anyhow::Result<()> {
    let selectors = check::Selectors {
        graph: input.graph,
        statements: input.statements,
        bindings: input.bindings,
        statement: input.statement,
    };
    let context = provenance_cli::repo_context::RepoContext::new(input.repo.as_str(), "default");
    check::check(
        context,
        input.strict,
        input.base,
        input.format.is_some(),
        selectors,
    )
    .await
}

/// Prints the shared domain guidance.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn prime_command(
    input: &provenance_porcelain::registry::PrimeInput,
) -> anyhow::Result<()> {
    prime::handle(input.format)
}

/// The typed input of one repository command, read from its declared flags.
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
    pub(super) fn from_matches(
        command: RepositoryCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = registry::Command::Repository(command).declaration();
        let input = |matches| crate::registry_cli::input(&declaration, matches);
        Ok(match command {
            RepositoryCommand::Init => Self::Init(input(matches)?),
            RepositoryCommand::CargoInit => Self::CargoInit(input(matches)?),
            RepositoryCommand::Materialize => Self::Materialize(input(matches)?),
            RepositoryCommand::Export => Self::Export(input(matches)?),
            RepositoryCommand::Import => Self::Import(input(matches)?),
            RepositoryCommand::MergeJsonl => Self::MergeJsonl(input(matches)?),
            RepositoryCommand::Validate => Self::Validate(input(matches)?),
            RepositoryCommand::SchemaShow => Self::SchemaShow(input(matches)?),
            RepositoryCommand::DictionaryImport => Self::DictionaryImport(input(matches)?),
            RepositoryCommand::DictionaryStatus => Self::DictionaryStatus(input(matches)?),
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
        RepositoryInput::SchemaShow(input) => schema::handle(cli::SchemaCommand::Show {
            artifact: artifact(&input.artifact)?,
            format: flag_value(&input.format, JsonFormat::from_flag)?,
        }),
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

/// The value that one declared flag names. The CLI parser accepts only declared values.
fn flag_value<T>(value: &str, parse: impl FnOnce(&str) -> Option<T>) -> anyhow::Result<T> {
    parse(value).ok_or_else(|| anyhow::anyhow!("unsupported value {value}"))
}

fn artifact(value: &str) -> anyhow::Result<cli::IdeationArtifactKind> {
    flag_value(value, |value| {
        <cli::IdeationArtifactKind as clap::ValueEnum>::from_str(value, false).ok()
    })
}

/// Runs one built-in command. The commands that wait on async work or on a
/// blocking thread run here. Every other command runs on the calling thread.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn dispatch(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::Docs { command } => docs::handle(command).await,
        Command::Review(options) => crate::review::run(options).await,
        command => dispatch_on_thread(command, quiet),
    }
}

/// Runs a synchronous handler on a blocking thread and waits for it.
async fn run_blocking(
    handler: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(handler).await?
}

/// Runs one built-in command whose handler does its work on the calling
/// thread.
#[cfg_attr(not(feature = "dogfood"), allow(unused_variables))]
fn dispatch_on_thread(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::Graph {
            requirement_id,
            context,
            ..
        } => {
            let repository = context.open()?;
            let graph = cache::get_requirement_graph(&repository, &StableId::new(requirement_id)?)?;
            output::print_json(&graph)
        }
        Command::Traceability {
            rule_id, context, ..
        } => {
            let repository = context.open()?;
            let trace = cache::trace_rule(&repository, &StableId::new(rule_id)?)?;
            output::print_json(&trace)
        }
        Command::Gaps { context, .. } => output::print_json(&cache::find_gaps(&context.open()?)?),
        Command::Health { context, .. } => {
            output::print_json(&cache::coverage_health(&context.open()?)?)
        }
        Command::Orphans { context, .. } => {
            output::print_json(&cache::orphan_rules(&context.open()?)?)
        }
        command => dispatch_remaining_on_thread(command, quiet),
    }
}

/// Runs a synchronous command that does not use the shared repository context.
#[cfg_attr(not(feature = "dogfood"), allow(unused_variables))]
fn dispatch_remaining_on_thread(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::GraphReference { command } => graph_reference::handle(command),
        Command::Coverage { command } => coverage::handle(command),
        Command::Report { command } => report::handle(command),
        Command::SwarmBacktrace { command } => swarm_backtrace::handle(command),
        Command::Skills { command } => skills::handle(command),
        #[cfg(feature = "dogfood")]
        Command::Dogfood { command } => dogfood::handle(command, quiet),
        // These commands run in `dispatch` or `dispatch_on_thread`.
        Command::Docs { .. }
        | Command::Review(_)
        | Command::Graph { .. }
        | Command::Traceability { .. }
        | Command::Gaps { .. }
        | Command::Health { .. }
        | Command::Orphans { .. } => {
            unreachable!("an earlier dispatcher runs this command")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: each value that a repository flag declares converts to the
    /// CLI value that its handler takes.
    #[test]
    fn every_declared_repository_value_converts() {
        for command in registry::Command::ALL {
            let registry::Command::Repository(repository) = command else {
                continue;
            };
            for flag in command.declaration().flags {
                for value in flag.values {
                    let converts = match flag.name {
                        "format" if *repository == RepositoryCommand::Export => {
                            OutputFormat::from_flag(value).is_some()
                        }
                        "format" => JsonFormat::from_flag(value).is_some(),
                        "artifact" => artifact(value).is_ok(),
                        "invocation-channel" => cli::InvocationChannel::from_flag(value).is_some(),
                        "package-manager" => cli::PackageManager::from_flag(value).is_some(),
                        other => panic!("{command:?} --{other} has no conversion"),
                    };
                    assert!(converts, "{command:?} --{} {value}", flag.name);
                }
            }
        }
    }
}
