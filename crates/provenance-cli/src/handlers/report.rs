use crate::cli::report::ReportCommand;
use crate::output::OutputFormat;
use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_report::envelope::ReportEnvelope;
use provenance_report::render;

pub(super) fn handle(command: ReportCommand) -> anyhow::Result<()> {
    let ReportCommand::Render {
        input,
        format,
        output,
    } = command;
    let raw = std::fs::read_to_string(&input).with_context(|| format!("failed to read {input}"))?;
    let envelope: ReportEnvelope = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse report envelope {input}"))?;
    envelope
        .validate()
        .map_err(|message| anyhow::anyhow!("invalid report envelope: {message}"))?;
    render::validate_duplicates(&envelope)
        .map_err(|message| anyhow::anyhow!("invalid report envelope: {message}"))?;
    let normalized = render::normalize(&envelope);
    match format {
        OutputFormat::Markdown => emit(&render::render_markdown(&normalized), output.as_ref()),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&normalized)?;
            emit(&json, output.as_ref())
        }
        other => {
            anyhow::bail!("unsupported format {other:?} for report render; use markdown or json")
        }
    }
}

fn emit(text: &str, output: Option<&Utf8PathBuf>) -> anyhow::Result<()> {
    match output {
        Some(path) => std::fs::write(path, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
