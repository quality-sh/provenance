use crate::cli::workspace::SkillsCommand;
use crate::output;
use crate::skills::BUNDLED;
use provenance_transport::skills;

pub(super) fn handle(command: SkillsCommand) -> anyhow::Result<()> {
    match command {
        SkillsCommand::List => output::print_json(&skills::list(BUNDLED)?)?,
        SkillsCommand::Show { name } => print!("{}", skills::show(BUNDLED, &name)?),
        SkillsCommand::Install {
            global,
            copy,
            force,
        } => output::print_json(&skills::install(BUNDLED, global, force, copy)?)?,
    }
    Ok(())
}
