use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, Parser as _};
use provenance_cli::porcelain::{self, OutputFormat};
use provenance_macros::rule;

/// One parsed command at the CLI boundary.
pub(super) enum Invocation {
    Builtin(Cli),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
}

pub(super) struct GlobalContext {
    pub repo: String,
    pub scope: String,
    pub quiet: bool,
}

pub(super) struct GetInvocation {
    repo: String,
    scope: String,
    format: Option<OutputFormat>,
    input: provenance_porcelain::get::GetInput,
}

struct SharedArguments {
    context: GlobalContext,
    format: Option<String>,
    words: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Builtin,
    Catalog,
    Get,
}

impl Invocation {
    pub(super) fn parse(arguments: Vec<String>) -> anyhow::Result<Self> {
        match select_route(&arguments)? {
            Route::Builtin => Ok(Self::Builtin(Cli::parse_from(arguments))),
            Route::Catalog => {
                let shared = SharedArguments::parse(&arguments)?;
                Ok(Self::Catalog(catalog_cli::Invocation::new(
                    shared.context,
                    shared.format,
                    shared.words,
                )?))
            }
            Route::Get => {
                let shared = SharedArguments::parse(&arguments)?;
                let format = match shared.format.as_deref() {
                    None => None,
                    Some("json") => Some(OutputFormat::Json),
                    Some(_) => anyhow::bail!("Porcelain get supports --format json"),
                };
                let words = shared.words.iter().map(String::as_str).collect::<Vec<_>>();
                let input = porcelain::parse_get(&words)
                    .map_err(|error| anyhow::anyhow!(error))?;
                Ok(Self::Get(GetInvocation {
                    repo: shared.context.repo,
                    scope: shared.context.scope,
                    format,
                    input,
                }))
            }
        }
    }

    pub(super) async fn dispatch(self) -> anyhow::Result<()> {
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
        }
    }
}

impl SharedArguments {
    fn parse(arguments: &[String]) -> anyhow::Result<Self> {
        let mut context = GlobalContext {
            repo: ".".to_owned(),
            scope: "default".to_owned(),
            quiet: false,
        };
        let mut format = None;
        let mut words = Vec::new();
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--quiet" => {
                    context.quiet = true;
                    index += 1;
                }
                "--repo" | "--scope" | "--format" => {
                    let value = arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("{} requires a value", arguments[index]))?;
                    match arguments[index].as_str() {
                        "--repo" => context.repo.clone_from(value),
                        "--scope" => context.scope.clone_from(value),
                        _ => format = Some(value.clone()),
                    }
                    index += 2;
                }
                "--stdin" | "--help" => {
                    words.push(arguments[index].clone());
                    index += 1;
                }
                flag if flag.starts_with("--") => {
                    words.push(flag.to_owned());
                    let value = arguments
                        .get(index + 1)
                        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
                    words.push(value.clone());
                    index += 2;
                }
                word => {
                    words.push(word.to_owned());
                    index += 1;
                }
            }
        }
        Ok(Self {
            context,
            format,
            words,
        })
    }
}

#[rule("rule_porcelain_get_is_default_action")]
fn select_route(arguments: &[String]) -> anyhow::Result<Route> {
    let Some((target_index, target)) = first_command_word(arguments)? else {
        return Ok(Route::Builtin);
    };
    if target.starts_with('-') {
        return Ok(Route::Builtin);
    }
    if next_command_word(arguments, target_index + 1)? == Some("get") {
        return Ok(Route::Get);
    }
    if catalog_cli::is_collection(target) {
        return Ok(Route::Catalog);
    }
    let builtin = Cli::command()
        .get_subcommands()
        .any(|candidate| candidate.get_name() == target);
    Ok(if builtin { Route::Builtin } else { Route::Get })
}

fn first_command_word(arguments: &[String]) -> anyhow::Result<Option<(usize, &str)>> {
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => {
                anyhow::ensure!(
                    arguments.get(index + 1).is_some(),
                    "{} requires a value",
                    arguments[index]
                );
                index += 2;
            }
            word => return Ok(Some((index, word))),
        }
    }
    Ok(None)
}

fn next_command_word(arguments: &[String], mut index: usize) -> anyhow::Result<Option<&str>> {
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => {
                anyhow::ensure!(
                    arguments.get(index + 1).is_some(),
                    "{} requires a value",
                    arguments[index]
                );
                index += 2;
            }
            word => return Ok(Some(word)),
        }
    }
    Ok(None)
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
        assert_eq!(
            select_route(&arguments(&["--repo", "repo", "sources", "list"])).unwrap(),
            Route::Catalog
        );
        assert_eq!(
            select_route(&arguments(&["sources", "get", "--repo", "repo"])).unwrap(),
            Route::Get
        );
        assert_eq!(
            select_route(&arguments(&["check", "--repo", "repo"])).unwrap(),
            Route::Builtin
        );
        assert_eq!(
            select_route(&arguments(&["unknown_id", "--format", "json"])).unwrap(),
            Route::Get
        );
    }

    #[test]
    fn local_flag_values_are_not_reparsed_as_globals() {
        let shared = SharedArguments::parse(&arguments(&[
            "sources", "create", "--name", "--repo", "--repo", "repository",
        ]))
        .unwrap();
        assert_eq!(shared.context.repo, "repository");
        assert_eq!(
            shared.words,
            ["sources", "create", "--name", "--repo"]
        );
    }
}
