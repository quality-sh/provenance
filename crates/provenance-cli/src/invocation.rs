use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, Parser as _};
use provenance_cli::porcelain::{self, OutputFormat};
use provenance_macros::rule;

/// One parsed command at the CLI boundary.
pub enum Invocation {
    Builtin(Cli),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
    Target(TargetInvocation),
}

pub struct GlobalContext {
    pub repo: String,
    pub scope: String,
    pub quiet: bool,
}

pub struct GetInvocation {
    repo: String,
    scope: String,
    format: Option<OutputFormat>,
    input: provenance_porcelain::get::GetInput,
}

pub struct TargetInvocation {
    context: GlobalContext,
    format: Option<OutputFormat>,
    target: String,
    action: provenance_transport::porcelain::Action,
    kind: Option<provenance_core::NodeType>,
    flags: Vec<String>,
    help: bool,
}

struct ExternalArguments<'a> {
    arguments: &'a [String],
    index: usize,
    context: GlobalContext,
    format: Option<String>,
    words: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandFamily {
    Builtin,
    Catalog,
    Get,
    Target,
}

impl Invocation {
    pub fn parse(arguments: Vec<String>) -> anyhow::Result<Self> {
        match CommandFamily::select(&arguments)? {
            CommandFamily::Builtin => Ok(Self::Builtin(Cli::parse_from(arguments))),
            CommandFamily::Catalog => {
                let shared = ExternalArguments::parse(&arguments)?;
                Ok(Self::Catalog(catalog_cli::Invocation::new(
                    shared.context,
                    shared.format.as_deref(),
                    shared.words,
                )?))
            }
            CommandFamily::Get => {
                let shared = ExternalArguments::parse(&arguments)?;
                let format = match shared.format.as_deref() {
                    None => None,
                    Some("json") => Some(OutputFormat::Json),
                    Some(_) => anyhow::bail!("Porcelain get supports --format json"),
                };
                let words = shared.words.iter().map(String::as_str).collect::<Vec<_>>();
                let input = porcelain::parse_get(&words).map_err(|error| anyhow::anyhow!(error))?;
                Ok(Self::Get(GetInvocation {
                    repo: shared.context.repo,
                    scope: shared.context.scope,
                    format,
                    input,
                }))
            }
            CommandFamily::Target => {
                let shared = ExternalArguments::parse(&arguments)?;
                Ok(Self::Target(TargetInvocation::parse(shared)?))
            }
        }
    }

    pub async fn dispatch(self) -> anyhow::Result<()> {
        match self {
            Self::Builtin(cli) => handlers::dispatch(cli.command, cli.quiet).await,
            Self::Catalog(invocation) => catalog_cli::dispatch(invocation).await,
            Self::Get(invocation) => {
                porcelain::dispatch_get(
                    &invocation.repo,
                    &invocation.scope,
                    invocation.format,
                    invocation.input,
                )
                .await
            }
            Self::Target(invocation) => invocation.dispatch().await,
        }
    }
}

impl CommandFamily {
    /// Selects one command grammar before any family parses its arguments.
    #[rule("rule_porcelain_cli_target_action_order")]
    #[rule("rule_porcelain_get_is_default_action")]
    fn select(arguments: &[String]) -> anyhow::Result<Self> {
        let target_index = after_shared_options(arguments, 1)?;
        let Some(target) = arguments.get(target_index) else {
            return Ok(Self::Builtin);
        };
        if target.starts_with('-') {
            return Ok(Self::Builtin);
        }
        let action_index = after_shared_options(arguments, target_index + 1)?;
        let explicit_get = arguments
            .get(action_index)
            .is_some_and(|word| word == "get");
        let explicit_target = arguments
            .get(action_index)
            .and_then(|word| provenance_transport::porcelain::Action::parse(word));
        let catalog = catalog_cli::is_collection(target);
        if catalog {
            return if explicit_get && collection_marker_selects_get(arguments, action_index)? {
                Ok(Self::Get)
            } else {
                Ok(Self::Catalog)
            };
        }
        if explicit_get {
            return Ok(Self::Get);
        }
        if explicit_target.is_some() {
            return Ok(Self::Target);
        }
        let builtin = Cli::command()
            .get_subcommands()
            .any(|candidate| candidate.get_name() == target);
        Ok(if builtin { Self::Builtin } else { Self::Get })
    }
}

impl TargetInvocation {
    fn parse(shared: SharedArguments) -> anyhow::Result<Self> {
        let mut words = shared.words.into_iter();
        let target = words
            .next()
            .ok_or_else(|| anyhow::anyhow!("target action requires a target"))?;
        let action_word = words
            .next()
            .ok_or_else(|| anyhow::anyhow!("target action requires an action"))?;
        let action = provenance_transport::porcelain::Action::parse(&action_word)
            .ok_or_else(|| anyhow::anyhow!("unsupported target action"))?;
        let (kind, flags, help) = target_fields(action, words.collect())?;
        let format = match shared.format.as_deref() {
            None => None,
            Some("json") => Some(OutputFormat::Json),
            Some(_) => anyhow::bail!("target actions support --format json"),
        };
        Ok(Self {
            context: shared.context,
            format,
            target,
            action,
            kind,
            flags,
            help,
        })
    }

    async fn dispatch(self) -> anyhow::Result<()> {
        if self.help {
            catalog_cli::print_target_help(self.action);
            return Ok(());
        }
        catalog_cli::dispatch_target(
            self.context,
            self.format,
            self.target,
            self.action,
            self.kind,
            self.flags,
        )
        .await
    }
}

fn target_fields(
    action: provenance_transport::porcelain::Action,
    words: Vec<String>,
) -> anyhow::Result<(Option<provenance_core::NodeType>, Vec<String>, bool)> {
    let mut kind = None;
    let mut flags = Vec::new();
    let mut help = false;
    let mut index = 0;
    while index < words.len() {
        match words[index].as_str() {
            "--help" => {
                help = true;
                index += 1;
            }
            "--stdin" => {
                flags.push(words[index].clone());
                index += 1;
            }
            flag => {
                let value = words
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
                if flag == "--type" {
                    anyhow::ensure!(kind.is_none(), "--type can be supplied once");
                    kind = Some(
                        provenance_core::NodeType::parse(value)
                            .map_err(|_| anyhow::anyhow!("unsupported record type"))?,
                    );
                } else {
                    flags.extend([words[index].clone(), value.clone()]);
                }
                index += 2;
            }
        }
    }
    if !help {
        anyhow::ensure!(
            (action == provenance_transport::porcelain::Action::Create) == kind.is_some(),
            "create requires --type and existing-record actions infer it"
        );
    }
    Ok((kind, flags, help))
}

/// A collection ID can itself be `get`. A following bare word is therefore
/// the catalog action slot; option syntax keeps `get` as the explicit view marker.
fn collection_marker_selects_get(
    arguments: &[String],
    marker_index: usize,
) -> anyhow::Result<bool> {
    let suffix_index = after_shared_options(arguments, marker_index + 1)?;
    Ok(arguments
        .get(suffix_index)
        .is_none_or(|word| word.starts_with('-')))
}

/// Skip shared option tokens for syntax selection only. The selected command
/// family remains the sole owner of their values and typed context.
fn after_shared_options(arguments: &[String], mut index: usize) -> anyhow::Result<usize> {
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => {
                anyhow::ensure!(
                    arguments.get(index + 1).is_some(),
                    "{argument} requires a value"
                );
                index += 2;
            }
            _ => break,
        }
    }
    Ok(index)
}

impl<'a> ExternalArguments<'a> {
    fn parse(arguments: &'a [String]) -> anyhow::Result<SharedArguments> {
        let mut parser = Self::new(arguments);
        parser.complete()?;
        Ok(parser.finish())
    }

    fn new(arguments: &'a [String]) -> Self {
        Self {
            arguments,
            index: 1,
            context: GlobalContext {
                repo: ".".to_owned(),
                scope: "default".to_owned(),
                quiet: false,
            },
            format: None,
            words: Vec::new(),
        }
    }

    fn complete(&mut self) -> anyhow::Result<()> {
        while self.index < self.arguments.len() {
            if self.take_global()? {
                continue;
            }
            match self.arguments[self.index].as_str() {
                "--stdin" | "--help" => {
                    self.words.push(self.arguments[self.index].clone());
                    self.index += 1;
                }
                flag if flag.starts_with("--") => {
                    self.words.push(flag.to_owned());
                    let value = self
                        .arguments
                        .get(self.index + 1)
                        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
                    self.words.push(value.clone());
                    self.index += 2;
                }
                word => {
                    self.words.push(word.to_owned());
                    self.index += 1;
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> SharedArguments {
        SharedArguments {
            context: self.context,
            format: self.format,
            words: self.words,
        }
    }

    fn take_global(&mut self) -> anyhow::Result<bool> {
        let Some(argument) = self.arguments.get(self.index) else {
            return Ok(false);
        };
        match argument.as_str() {
            "--quiet" => {
                self.context.quiet = true;
                self.index += 1;
            }
            "--repo" | "--scope" | "--format" => {
                let value = self
                    .arguments
                    .get(self.index + 1)
                    .ok_or_else(|| anyhow::anyhow!("{argument} requires a value"))?;
                match argument.as_str() {
                    "--repo" => self.context.repo.clone_from(value),
                    "--scope" => self.context.scope.clone_from(value),
                    _ => self.format = Some(value.clone()),
                }
                self.index += 2;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

struct SharedArguments {
    context: GlobalContext,
    format: Option<String>,
    words: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<String> {
        std::iter::once("provenance")
            .chain(words.iter().copied())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn route_is_one_deterministic_grammar_decision() {
        let route = |words: &[&str]| CommandFamily::select(&arguments(words)).unwrap();
        assert_eq!(route(&["--help"]), CommandFamily::Builtin);
        assert_eq!(
            route(&["--quiet", "check", "--repo", "repo"]),
            CommandFamily::Builtin
        );
        assert!(matches!(
            route(&["--repo", "repo", "sources", "list"]),
            CommandFamily::Catalog
        ));
        assert!(matches!(
            route(&["sources", "get", "--repo", "repo"]),
            CommandFamily::Get
        ));
        assert!(matches!(
            route(&["sources", "get", "update", "--repo", "repo"]),
            CommandFamily::Catalog
        ));
        assert_eq!(
            route(&["sources", "get", "get", "--repo", "repo"]),
            CommandFamily::Catalog
        );
        assert_eq!(
            route(&["sources", "update", "get", "--repo", "repo"]),
            CommandFamily::Catalog
        );
        assert_eq!(
            route(&["sources", "get", "--unknown-option", "value"]),
            CommandFamily::Get
        );
        assert_eq!(
            route(&["sources", "--help", "--repo", "repo"]),
            CommandFamily::Catalog
        );
        assert!(matches!(
            route(&["check", "--repo", "repo"]),
            CommandFamily::Builtin
        ));
        assert!(matches!(
            route(&["check", "get", "--repo", "repo"]),
            CommandFamily::Get
        ));
        assert_eq!(
            route(&["check", "--repo", "repo", "get", "--format", "json"]),
            CommandFamily::Get
        );
        assert!(matches!(
            route(&["check", "--strict"]),
            CommandFamily::Builtin
        ));
        assert!(matches!(
            route(&["--repo", "repo", "unknown_id", "--format", "json"]),
            CommandFamily::Get
        ));
        assert_eq!(
            route(&["--repo", "repo", "unknown_id", "update"]),
            CommandFamily::Target
        );
        assert_eq!(
            route(&[
                "unknown_id",
                "--scope",
                "other",
                "create",
                "--type",
                "source"
            ]),
            CommandFamily::Target
        );
    }

    #[test]
    fn local_flag_values_are_not_reparsed_as_globals() {
        let input = arguments(&[
            "sources",
            "create",
            "--name",
            "--repo",
            "--repo",
            "repository",
        ]);
        assert_eq!(
            CommandFamily::select(&input).unwrap(),
            CommandFamily::Catalog
        );
        let shared = ExternalArguments::parse(&input).unwrap();
        assert_eq!(shared.context.repo, "repository");
        assert_eq!(shared.words, ["sources", "create", "--name", "--repo"]);
    }
}
