use crate::cli::workspace::SkillsCommand;
use crate::output;
use crate::skills;

pub(super) fn handle(command: SkillsCommand) -> anyhow::Result<()> {
    match command {
        SkillsCommand::List { .. } => output::print_json(&skills::list()?)?,
        SkillsCommand::Show { name } => print!("{}", skills::show(&name)?),
        SkillsCommand::Install {
            global,
            copy,
            force,
            ..
        } => output::print_json(&skills::install(global, force, copy)?)?,
    }
    Ok(())
}
