//! The skills that this program bundles. The build script copies their sources into
//! the build output; the setup adapter of provenance-transport installs them.

use provenance_transport::skills::EmbeddedSkill;

macro_rules! bundled {
    ($($directory:literal),+ $(,)?) => {
        &[$(EmbeddedSkill {
            directory: $directory,
            content: include_str!(concat!(env!("OUT_DIR"), "/skills/", $directory, "/SKILL.md")),
        }),+]
    };
}

/// The bundled skills, in the order of their directory names.
pub const BUNDLED: &[EmbeddedSkill] = bundled!(
    "provenance-fork-tournament",
    "provenance-grounded-writing",
    "provenance-shaping",
    "provenance-swarm-backtrace",
);
