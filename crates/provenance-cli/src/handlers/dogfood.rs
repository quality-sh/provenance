//! Dev-build-only capture of agent pain-point notes about provenance itself. The
//! spool adapter of provenance-transport does the work (see docs/dogfood.md); this
//! module reads the input, checks the surface name, and prints the result.

use crate::output;
use provenance_porcelain::dogfood::Enrichment;
use provenance_porcelain::registry::{
    self, DogfoodCommand, DogfoodListInput, DogfoodNoteInput, DogfoodReportInput,
    DogfoodSimulateInput, TriageHandleInput, TriageListInput, TriageReopenInput,
};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::SpoolPort;
use std::io::Read;

/// The typed input of one dogfood command, read from its declared flags.
pub enum DogfoodInput {
    Note(DogfoodNoteInput),
    List(DogfoodListInput),
    Report(DogfoodReportInput),
    TriageHandle(TriageHandleInput),
    TriageReopen(TriageReopenInput),
    TriageList(TriageListInput),
    Simulate(DogfoodSimulateInput),
}

impl DogfoodInput {
    /// Reads the input of `command` from the matches of its parser.
    pub fn from_matches(
        command: DogfoodCommand,
        matches: &clap::ArgMatches,
    ) -> anyhow::Result<Self> {
        let declaration = registry::Command::Dogfood(command).declaration();
        macro_rules! read {
            ($variant:ident) => {
                Self::$variant(crate::registry_cli::input(&declaration, matches)?)
            };
        }
        Ok(match command {
            DogfoodCommand::Note => read!(Note),
            DogfoodCommand::List => read!(List),
            DogfoodCommand::Report => read!(Report),
            DogfoodCommand::TriageHandle => read!(TriageHandle),
            DogfoodCommand::TriageReopen => read!(TriageReopen),
            DogfoodCommand::TriageList => read!(TriageList),
            DogfoodCommand::Simulate => read!(Simulate),
        })
    }
}

pub async fn handle(input: DogfoodInput, quiet: bool) -> anyhow::Result<()> {
    let work = Porcelain::new(SpoolPort);
    match input {
        DogfoodInput::Note(input) => {
            check_surface(&input.surface)?;
            let recorded = work.dogfood_note(input).await?;
            if !quiet {
                println!("recorded dogfood note in {}", recorded.spool);
            }
            Ok(())
        }
        DogfoodInput::List(input) => {
            super::flag_value(&input.format, output::JsonFormat::from_flag)?;
            output::print_json(&work.dogfood_list(input).await?)
        }
        DogfoodInput::Report(input) => {
            super::flag_value(&input.format, output::JsonFormat::from_flag)?;
            let enrichment = enrichment(input.enrich.as_deref())?;
            output::print_json(&work.dogfood_report(input, enrichment).await?)
        }
        DogfoodInput::TriageHandle(input) => {
            print_marked(&work.dogfood_triage_handle(input).await?, quiet);
            Ok(())
        }
        DogfoodInput::TriageReopen(input) => {
            print_marked(&work.dogfood_triage_reopen(input).await?, quiet);
            Ok(())
        }
        DogfoodInput::TriageList(input) => {
            super::flag_value(&input.format, output::JsonFormat::from_flag)?;
            output::print_json(&work.dogfood_triage_list(input).await?)
        }
        DogfoodInput::Simulate(input) => {
            print!("{}", work.dogfood_simulate(input).await?.text);
            Ok(())
        }
    }
}

/// Refuses a surface that is not a top-level command name or "general".
fn check_surface(surface: &str) -> anyhow::Result<()> {
    let surfaces = valid_surfaces();
    if !surfaces.iter().any(|known| known == surface) {
        anyhow::bail!(
            "unknown surface `{surface}`; valid surfaces: {}",
            surfaces.join(", ")
        );
    }
    Ok(())
}

/// Surfaces are the CLI's own top-level subcommand names plus "general",
/// read from the root parser so the set never drifts from the actual command tree.
fn valid_surfaces() -> Vec<String> {
    let mut surfaces: Vec<String> = crate::invocation::root_command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    surfaces.push("general".to_string());
    surfaces.sort();
    surfaces
}

/// The enrichment that `--enrich` names: a file path, or `-` for standard input.
fn enrichment(enrich: Option<&str>) -> anyhow::Result<Enrichment> {
    Ok(match enrich {
        None => Enrichment::None,
        Some("-") => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|error| anyhow::anyhow!(error).context("reading enrichment from stdin"))?;
            Enrichment::Text(buffer)
        }
        Some(path) => Enrichment::File(path.to_owned()),
    })
}

fn print_marked(marked: &provenance_porcelain::dogfood::TriageMarked, quiet: bool) {
    if !quiet {
        let status = if marked.handled {
            "handled"
        } else {
            "unhandled"
        };
        println!("marked {} {status}", marked.id);
    }
}
