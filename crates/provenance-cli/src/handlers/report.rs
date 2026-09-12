use crate::cli::report::ReportCommand;
use crate::output::OutputFormat;
use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_report::envelope::ReportEnvelope;
use provenance_report::render;

pub(super) fn handle(command: ReportCommand) -> anyhow::Result<()> {
    match command {
        ReportCommand::Render {
            input,
            format,
            output,
        } => render_handler(&input, format, &output),
        ReportCommand::Build {
            repo,
            base,
            head,
            repository,
            scope,
            path,
            output,
        } => {
            let scan_path = path.as_deref().unwrap_or(repo.as_path());
            let envelope =
                provenance_report::build::build_envelope(&provenance_report::build::BuildInput {
                    repo: repo.as_path(),
                    scan_path,
                    scope: &scope,
                    base: &base,
                    head: &head,
                    repository: &repository,
                })?;
            let json = serde_json::to_string_pretty(&envelope)?;
            emit(&json, output.as_ref())
        }
    }
}

fn render_handler(
    input: &Utf8PathBuf,
    format: OutputFormat,
    output: &Option<Utf8PathBuf>,
) -> anyhow::Result<()> {
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
