use crate::envelope::ReportEnvelope;
use crate::render;
use std::fmt;

/// A supported report envelope output format.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RenderFormat {
    Json,
    Markdown,
}

/// A report envelope parse, validation, or encoding error.
#[derive(Debug)]
pub enum RenderError {
    Parse(serde_json::Error),
    Invalid(String),
    Encode(serde_json::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(source) => write!(formatter, "failed to parse report envelope: {source}"),
            Self::Invalid(message) => write!(formatter, "invalid report envelope: {message}"),
            Self::Encode(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(source) | Self::Encode(source) => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

/// Parses, validates, normalizes, and renders one report envelope.
pub fn render_envelope(raw: &str, format: RenderFormat) -> Result<String, RenderError> {
    let envelope: ReportEnvelope = serde_json::from_str(raw).map_err(RenderError::Parse)?;
    envelope.validate().map_err(RenderError::Invalid)?;
    render::validate_duplicates(&envelope).map_err(RenderError::Invalid)?;
    let normalized = render::normalize(&envelope);

    match format {
        RenderFormat::Json => {
            serde_json::to_string_pretty(&normalized).map_err(RenderError::Encode)
        }
        RenderFormat::Markdown => Ok(render::render_markdown(&normalized)),
    }
}
