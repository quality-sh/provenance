use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, Parser as _};
use provenance_cli::porcelain::{self, OutputFormat};
use provenance_macros::rule;

/// One parsed command at the CLI boundary.
pub enum Invocation {
    Builtin(Cli),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
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

struct ArgumentParser<'a> {
    arguments: &'a [String],
    index: usize,
    context: GlobalContext,
    format: Option<String>,
    words: Vec<String>,
}

#[derive(Debug)]
enum Route {
    Builtin,
    Catalog,
    Get(provenance_porcelain::get::GetInput),
}

impl Invocation {
    pub fn parse(arguments: Vec<String>) -> anyhow::Result<Self> {
        let mut parser = ArgumentParser::new(&arguments);
        match parser.route()? {
            Route::Builtin => Ok(Self::Builtin(Cli::parse_from(arguments))),
            Route::Catalog => {
                let shared = parser.finish()?;
                Ok(Self::Catalog(catalog_cli::Invocation::new(
                    shared.context,
                    shared.format.as_deref(),
                    shared.words,
                )?))
            }
            Route::Get(input) => {
                let shared = parser.finish()?;
                let format = match shared.format.as_deref() {
                    None => None,
                    Some("json") => Some(OutputFormat::Json),
                    Some(_) => anyhow::bail!("Porcelain get supports --format json"),
                };
                Ok(Self::Get(GetInvocation {
                    repo: shared.context.repo,
                    scope: shared.context.scope,
                    format,
                    input,
                }))
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
        }
    }
}

impl<'a> ArgumentParser<'a> {
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

    /// Selects one command grammar before repository state is read.
    #[rule("rule_porcelain_get_is_default_action")]
    fn route(&mut self) -> anyhow::Result<Route> {
        self.take_globals()?;
        let Some(target) = self.arguments.get(self.index).cloned() else {
            return Ok(Route::Builtin);
        };
        if target.starts_with('-') {
            return Ok(Route::Builtin);
        }
        self.words.push(target.clone());
        self.index += 1;
        self.take_globals()?;
        let catalog = catalog_cli::is_collection(&target);
        let builtin = Cli::command()
            .get_subcommands()
            .any(|candidate| candidate.get_name() == target);
        let explicit_get = self
            .arguments
            .get(self.index)
            .is_some_and(|word| word == "get");
        if explicit_get {
            self.complete()?;
            let words = self.words.iter().map(String::as_str).collect::<Vec<_>>();
            match porcelain::parse_get(&words) {
                Ok(input) => return Ok(Route::Get(input)),
                Err(_) if catalog => return Ok(Route::Catalog),
                Err(_) if builtin => return Ok(Route::Builtin),
                Err(error) => return Err(anyhow::anyhow!(error)),
            }
        }
        if catalog {
            return Ok(Route::Catalog);
        }
        if builtin {
            return Ok(Route::Builtin);
        }
        self.complete()?;
        let words = self.words.iter().map(String::as_str).collect::<Vec<_>>();
        let input = porcelain::parse_get(&words).map_err(|error| anyhow::anyhow!(error))?;
        Ok(Route::Get(input))
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

    fn finish(mut self) -> anyhow::Result<SharedArguments> {
        self.complete()?;
        Ok(SharedArguments {
            context: self.context,
            format: self.format,
            words: self.words,
        })
    }

    fn take_globals(&mut self) -> anyhow::Result<()> {
        while self.take_global()? {}
        Ok(())
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
        let route = |words: &[&str]| {
            let input = arguments(words);
            ArgumentParser::new(&input).route().unwrap()
        };
        assert!(matches!(
            route(&["--repo", "repo", "sources", "list"]),
            Route::Catalog
        ));
        assert!(matches!(
            route(&["sources", "get", "--repo", "repo"]),
            Route::Get(_)
        ));
        assert!(matches!(
            route(&["sources", "get", "update", "--repo", "repo"]),
            Route::Catalog
        ));
        assert!(matches!(
            route(&["check", "--repo", "repo"]),
            Route::Builtin
        ));
        assert!(matches!(
            route(&["check", "get", "--repo", "repo"]),
            Route::Get(_)
        ));
        assert!(matches!(route(&["check", "--strict"]), Route::Builtin));
        assert!(matches!(
            route(&["unknown_id", "--format", "json"]),
            Route::Get(_)
        ));
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
        let mut parser = ArgumentParser::new(&input);
        assert!(matches!(parser.route().unwrap(), Route::Catalog));
        let shared = parser.finish().unwrap();
        assert_eq!(shared.context.repo, "repository");
        assert_eq!(shared.words, ["sources", "create", "--name", "--repo"]);
    }
}
