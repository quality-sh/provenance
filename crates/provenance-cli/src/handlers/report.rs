use crate::cli::report::ReportCommand;
use crate::output::ReportFormat;
use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_report::{render_envelope, RenderError, RenderFormat};

pub(super) fn handle(command: ReportCommand) -> anyhow::Result<()> {
    match command {
        ReportCommand::Render {
            input,
            format,
            output,
        } => render_handler(&input, format, output.as_ref()),
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
    format: ReportFormat,
    output: Option<&Utf8PathBuf>,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(input).with_context(|| format!("failed to read {input}"))?;
    let format = match format {
        ReportFormat::Json => RenderFormat::Json,
        ReportFormat::Markdown => RenderFormat::Markdown,
    };
    let rendered = match render_envelope(&raw, format) {
        Ok(rendered) => rendered,
        Err(RenderError::Parse(source)) => {
            return Err(anyhow::Error::new(source))
                .with_context(|| format!("failed to parse report envelope {input}"));
        }
        Err(error) => return Err(error.into()),
    };
    emit(&rendered, output)
}

fn emit(text: &str, output: Option<&Utf8PathBuf>) -> anyhow::Result<()> {
    match output {
        Some(path) => std::fs::write(path, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
